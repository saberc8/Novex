import json
import unittest

from parser_worker.worker import (
    ParserJobMessage,
    ParserWorkerConfig,
    acquire_job_lease,
    handle_parser_message,
    mark_job_idempotency,
    release_job_lease,
)


class ParserWorkerQueueTest(unittest.TestCase):
    def test_parser_job_message_parses_backend_outbox_payload(self):
        message = ParserJobMessage.from_json(
            json.dumps(
                {
                    "outboxId": 101,
                    "tenantId": 1,
                    "datasetId": 7,
                    "documentId": 42,
                    "parserJobId": 99,
                    "attempt": 1,
                    "maxAttempts": 5,
                    "parserRequest": {
                        "tenantId": 1,
                        "datasetId": 7,
                        "documentId": 42,
                        "parserJobId": 99,
                        "source": {
                            "kind": "inlineText",
                            "contentType": "text/markdown",
                            "name": "handbook.md",
                            "content": "# Handbook",
                            "sourceHash": "abc123",
                        },
                    },
                }
            ).encode("utf-8")
        )

        self.assertEqual(message.outbox_id, 101)
        self.assertEqual(message.parser_job_id, 99)
        self.assertEqual(message.attempt, 1)
        self.assertEqual(message.max_attempts, 5)
        self.assertEqual(message.parser_request["source"]["sourceHash"], "abc123")
        self.assertEqual(message.to_payload()["parserJobId"], 99)

    def test_parser_job_message_rejects_invalid_identifiers(self):
        with self.assertRaisesRegex(ValueError, "parserJobId"):
            ParserJobMessage.from_dict(
                {
                    "outboxId": 101,
                    "tenantId": 1,
                    "datasetId": 7,
                    "documentId": 42,
                    "parserJobId": 0,
                    "attempt": 1,
                    "maxAttempts": 5,
                    "parserRequest": {},
                }
            )

    def test_redis_lease_is_acquired_once_and_only_owner_releases(self):
        redis = FakeRedis()
        message = example_message()

        self.assertTrue(acquire_job_lease(redis, message, worker_id="worker-a", ttl_seconds=30))
        self.assertFalse(acquire_job_lease(redis, message, worker_id="worker-b", ttl_seconds=30))
        self.assertFalse(release_job_lease(redis, message, worker_id="worker-b"))
        self.assertTrue(release_job_lease(redis, message, worker_id="worker-a"))
        self.assertTrue(acquire_job_lease(redis, message, worker_id="worker-b", ttl_seconds=30))

    def test_idempotency_key_skips_duplicate_source_hash(self):
        redis = FakeRedis()
        message = example_message()

        self.assertTrue(mark_job_idempotency(redis, message, ttl_seconds=300))
        self.assertFalse(mark_job_idempotency(redis, message, ttl_seconds=300))

    def test_handle_native_success_acks_without_retry(self):
        redis = FakeRedis()
        publisher = FakePublisher()
        runner = FakeRunner({"status": "succeeded"})

        outcome = handle_parser_message(
            example_message().to_payload(),
            redis_client=redis,
            publisher=publisher,
            worker_id="worker-a",
            backend_base_url="http://backend.local",
            backend_token="token-1",
            runner=runner,
        )

        self.assertTrue(outcome.ack)
        self.assertEqual(outcome.status, "succeeded")
        self.assertEqual(len(runner.calls), 1)
        self.assertEqual(runner.calls[0]["backend_base_url"], "http://backend.local")
        self.assertEqual(runner.calls[0]["backend_token"], "token-1")
        self.assertEqual(publisher.retry_messages, [])
        self.assertEqual(publisher.dead_messages, [])

    def test_handle_submitted_mineru_task_republishes_retry_message(self):
        redis = FakeRedis()
        publisher = FakePublisher()
        runner = FakeRunner(
            {
                "status": "submitted",
                "parserResult": {
                    "mineruTask": {
                        "taskId": "task-1",
                        "state": "pending",
                        "fullZipUrl": "",
                    }
                },
            }
        )

        outcome = handle_parser_message(
            example_message().to_payload(),
            redis_client=redis,
            publisher=publisher,
            worker_id="worker-a",
            backend_base_url="http://backend.local",
            backend_token="token-1",
            runner=runner,
        )

        self.assertTrue(outcome.ack)
        self.assertEqual(outcome.status, "retry_published")
        self.assertEqual(len(publisher.retry_messages), 1)
        retry = publisher.retry_messages[0]
        self.assertEqual(retry.attempt, 2)
        self.assertEqual(retry.max_attempts, 5)
        self.assertEqual(retry.parser_request["mineruTask"]["taskId"], "task-1")

    def test_handle_submitted_mineru_task_keeps_polling_after_max_attempts(self):
        redis = FakeRedis()
        publisher = FakePublisher()
        runner = FakeRunner(
            {
                "status": "submitted",
                "parserResult": {
                    "mineruTask": {
                        "taskId": "batch:batch-1",
                        "state": "running",
                        "fullZipUrl": "",
                    }
                },
            }
        )

        outcome = handle_parser_message(
            example_message(attempt=5, maxAttempts=5).to_payload(),
            redis_client=redis,
            publisher=publisher,
            worker_id="worker-a",
            backend_base_url="http://backend.local",
            backend_token="token-1",
            runner=runner,
        )

        self.assertTrue(outcome.ack)
        self.assertEqual(outcome.status, "retry_published")
        self.assertEqual(len(publisher.retry_messages), 1)
        self.assertEqual(publisher.dead_messages, [])
        self.assertEqual(publisher.retry_messages[0].attempt, 6)
        self.assertEqual(publisher.retry_messages[0].parser_request["mineruTask"]["taskId"], "batch:batch-1")

    def test_handle_runner_exception_republishes_retry_message(self):
        redis = FakeRedis()
        publisher = FakePublisher()
        runner = FakeRunner(error=RuntimeError("backend callback failed"))

        outcome = handle_parser_message(
            example_message().to_payload(),
            redis_client=redis,
            publisher=publisher,
            worker_id="worker-a",
            backend_base_url="http://backend.local",
            backend_token="token-1",
            runner=runner,
        )

        self.assertTrue(outcome.ack)
        self.assertEqual(outcome.status, "retry_published")
        self.assertEqual(len(publisher.retry_messages), 1)
        self.assertEqual(publisher.retry_messages[0].attempt, 2)
        self.assertIn("backend callback failed", publisher.retry_messages[0].parser_request["lastError"])
        self.assertEqual(publisher.dead_messages, [])

    def test_handle_mineru_poll_exception_keeps_retrying_after_max_attempts(self):
        redis = FakeRedis()
        publisher = FakePublisher()

        def mineru_runner(_request, *, task_id, backend_base_url, backend_token):
            self.assertEqual(task_id, "batch:batch-1")
            raise RuntimeError("MinerU network error")

        outcome = handle_parser_message(
            example_message(
                attempt=5,
                maxAttempts=5,
                parserRequest={
                    "tenantId": 1,
                    "datasetId": 7,
                    "documentId": 42,
                    "parserJobId": 99,
                    "source": {"kind": "objectStorage", "uri": "/file/knowledge/1.pdf"},
                    "mineruTask": {
                        "taskId": "batch:batch-1",
                        "state": "running",
                        "fullZipUrl": "",
                    },
                },
            ).to_payload(),
            redis_client=redis,
            publisher=publisher,
            worker_id="worker-a",
            backend_base_url="http://backend.local",
            backend_token="token-1",
            mineru_runner=mineru_runner,
        )

        self.assertTrue(outcome.ack)
        self.assertEqual(outcome.status, "retry_published")
        self.assertEqual(len(publisher.retry_messages), 1)
        self.assertEqual(publisher.dead_messages, [])
        self.assertIn("MinerU network error", publisher.retry_messages[0].parser_request["lastError"])

    def test_handle_exhausted_attempts_publishes_dead_message(self):
        redis = FakeRedis()
        publisher = FakePublisher()
        runner = FakeRunner(error=RuntimeError("backend callback failed"))

        outcome = handle_parser_message(
            example_message(attempt=5, maxAttempts=5).to_payload(),
            redis_client=redis,
            publisher=publisher,
            worker_id="worker-a",
            backend_base_url="http://backend.local",
            backend_token="token-1",
            runner=runner,
        )

        self.assertTrue(outcome.ack)
        self.assertEqual(outcome.status, "dead_published")
        self.assertEqual(publisher.retry_messages, [])
        self.assertEqual(len(publisher.dead_messages), 1)
        self.assertEqual(publisher.dead_messages[0].attempt, 5)
        self.assertIn("backend callback failed", publisher.dead_messages[0].parser_request["lastError"])

    def test_worker_config_reads_parser_queue_env_and_masks_secrets(self):
        config = ParserWorkerConfig.from_env(
            {
                "PARSER_WORKER_ID": "worker-7",
                "PARSER_BACKEND_BASE_URL": "http://backend:4398",
                "PARSER_BACKEND_TOKEN": "service-token-1234",
                "RABBITMQ_URL": "amqp://guest:guest@rabbitmq:5672/%2f",
                "REDIS_URL": "redis://redis:6379/0",
                "RABBITMQ_PARSER_EXCHANGE": "novex.parser",
                "RABBITMQ_PARSER_EXECUTE_QUEUE": "novex.parser.execute",
                "RABBITMQ_PARSER_RETRY_ROUTING_KEY": "parser.retry",
                "RABBITMQ_PARSER_DEAD_ROUTING_KEY": "parser.dead",
                "PARSER_WORKER_PREFETCH": "8",
                "PARSER_WORKER_LEASE_TTL_SECONDS": "120",
            }
        )

        self.assertEqual(config.worker_id, "worker-7")
        self.assertEqual(config.backend_base_url, "http://backend:4398")
        self.assertEqual(config.rabbitmq_url, "amqp://guest:guest@rabbitmq:5672/%2f")
        self.assertEqual(config.redis_url, "redis://redis:6379/0")
        self.assertEqual(config.exchange, "novex.parser")
        self.assertEqual(config.execute_queue, "novex.parser.execute")
        self.assertEqual(config.retry_routing_key, "parser.retry")
        self.assertEqual(config.dead_routing_key, "parser.dead")
        self.assertEqual(config.prefetch_count, 8)
        self.assertEqual(config.lease_ttl_seconds, 120)
        self.assertEqual(config.masked_backend_token, "serv****1234")


class FakeRedis:
    def __init__(self):
        self.values = {}

    def set(self, key, value, nx=False, ex=None):
        if nx and key in self.values:
            return False
        self.values[key] = value
        return True

    def get(self, key):
        return self.values.get(key)

    def delete(self, key):
        existed = key in self.values
        self.values.pop(key, None)
        return 1 if existed else 0


class FakePublisher:
    def __init__(self):
        self.retry_messages = []
        self.dead_messages = []

    def publish_retry(self, message):
        self.retry_messages.append(message)

    def publish_dead(self, message):
        self.dead_messages.append(message)


class FakeRunner:
    def __init__(self, result=None, error=None):
        self.result = result or {"status": "succeeded"}
        self.error = error
        self.calls = []

    def __call__(self, request, *, backend_base_url, backend_token):
        self.calls.append(
            {
                "request": request,
                "backend_base_url": backend_base_url,
                "backend_token": backend_token,
            }
        )
        if self.error:
            raise self.error
        return self.result


def example_message(**overrides):
    payload = {
        "outboxId": 101,
        "tenantId": 1,
        "datasetId": 7,
        "documentId": 42,
        "parserJobId": 99,
        "attempt": 1,
        "maxAttempts": 5,
        "parserRequest": {
            "tenantId": 1,
            "datasetId": 7,
            "documentId": 42,
            "parserJobId": 99,
            "source": {
                "kind": "inlineText",
                "contentType": "text/markdown",
                "name": "handbook.md",
                "content": "# Handbook",
                "sourceHash": "abc123",
            },
        },
    }
    payload.update(overrides)
    return ParserJobMessage.from_dict(payload)


if __name__ == "__main__":
    unittest.main()
