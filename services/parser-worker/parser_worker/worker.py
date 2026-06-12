from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping
import json
from os import environ


DEFAULT_RABBITMQ_URL = "amqp://guest:guest@127.0.0.1:5673/%2f"
DEFAULT_REDIS_URL = "redis://127.0.0.1:16379/0"
DEFAULT_EXCHANGE = "novex.parser"
DEFAULT_EXECUTE_QUEUE = "novex.parser.execute"
DEFAULT_RETRY_QUEUE = "novex.parser.retry"
DEFAULT_DEAD_QUEUE = "novex.parser.dead"
DEFAULT_EXECUTE_ROUTING_KEY = "parser.execute"
DEFAULT_RETRY_ROUTING_KEY = "parser.retry"
DEFAULT_DEAD_ROUTING_KEY = "parser.dead"
DEFAULT_RETRY_TTL_MS = 30000
DEFAULT_PREFETCH_COUNT = 4
DEFAULT_LEASE_TTL_SECONDS = 300
DEFAULT_IDEMPOTENCY_TTL_SECONDS = 86400


LEASE_KEY_PREFIX = "novex:parser:lease"
IDEMPOTENCY_KEY_PREFIX = "novex:parser:idempotency"


@dataclass(frozen=True)
class ParserWorkerConfig:
    worker_id: str
    backend_base_url: str
    backend_token: str
    rabbitmq_url: str
    redis_url: str
    exchange: str
    execute_queue: str
    retry_queue: str
    dead_queue: str
    execute_routing_key: str
    retry_routing_key: str
    dead_routing_key: str
    retry_ttl_ms: int
    prefetch_count: int
    lease_ttl_seconds: int
    idempotency_ttl_seconds: int

    @classmethod
    def from_env(cls, env: Mapping[str, str] | None = None) -> "ParserWorkerConfig":
        values = env if env is not None else environ
        return cls(
            worker_id=non_empty(values.get("PARSER_WORKER_ID"), "parser-worker"),
            backend_base_url=non_empty(values.get("PARSER_BACKEND_BASE_URL"), ""),
            backend_token=non_empty(values.get("PARSER_BACKEND_TOKEN"), ""),
            rabbitmq_url=non_empty(values.get("RABBITMQ_URL"), DEFAULT_RABBITMQ_URL),
            redis_url=non_empty(values.get("REDIS_URL"), DEFAULT_REDIS_URL),
            exchange=non_empty(values.get("RABBITMQ_PARSER_EXCHANGE"), DEFAULT_EXCHANGE),
            execute_queue=non_empty(values.get("RABBITMQ_PARSER_EXECUTE_QUEUE"), DEFAULT_EXECUTE_QUEUE),
            retry_queue=non_empty(values.get("RABBITMQ_PARSER_RETRY_QUEUE"), DEFAULT_RETRY_QUEUE),
            dead_queue=non_empty(values.get("RABBITMQ_PARSER_DEAD_QUEUE"), DEFAULT_DEAD_QUEUE),
            execute_routing_key=non_empty(
                values.get("RABBITMQ_PARSER_EXECUTE_ROUTING_KEY"),
                DEFAULT_EXECUTE_ROUTING_KEY,
            ),
            retry_routing_key=non_empty(
                values.get("RABBITMQ_PARSER_RETRY_ROUTING_KEY"),
                DEFAULT_RETRY_ROUTING_KEY,
            ),
            dead_routing_key=non_empty(
                values.get("RABBITMQ_PARSER_DEAD_ROUTING_KEY"),
                DEFAULT_DEAD_ROUTING_KEY,
            ),
            retry_ttl_ms=parse_positive_int(values.get("RABBITMQ_PARSER_RETRY_TTL_MS"), DEFAULT_RETRY_TTL_MS),
            prefetch_count=parse_positive_int(values.get("PARSER_WORKER_PREFETCH"), DEFAULT_PREFETCH_COUNT),
            lease_ttl_seconds=parse_positive_int(
                values.get("PARSER_WORKER_LEASE_TTL_SECONDS"),
                DEFAULT_LEASE_TTL_SECONDS,
            ),
            idempotency_ttl_seconds=parse_positive_int(
                values.get("PARSER_WORKER_IDEMPOTENCY_TTL_SECONDS"),
                DEFAULT_IDEMPOTENCY_TTL_SECONDS,
            ),
        )

    @property
    def masked_backend_token(self) -> str:
        return mask_secret(self.backend_token)


@dataclass(frozen=True)
class ParserJobMessage:
    outbox_id: int
    tenant_id: int
    dataset_id: int
    document_id: int
    parser_job_id: int
    attempt: int
    max_attempts: int
    parser_request: Mapping[str, Any]

    @classmethod
    def from_json(cls, value: bytes | str) -> "ParserJobMessage":
        if isinstance(value, bytes):
            value = value.decode("utf-8")
        return cls.from_dict(json.loads(value))

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "ParserJobMessage":
        parser_request = value.get("parserRequest")
        if not isinstance(parser_request, Mapping):
            raise ValueError("parserRequest must be an object")
        return cls(
            outbox_id=positive_int(value.get("outboxId"), "outboxId"),
            tenant_id=positive_int(value.get("tenantId"), "tenantId"),
            dataset_id=positive_int(value.get("datasetId"), "datasetId"),
            document_id=positive_int(value.get("documentId"), "documentId"),
            parser_job_id=positive_int(value.get("parserJobId"), "parserJobId"),
            attempt=positive_int(value.get("attempt"), "attempt"),
            max_attempts=positive_int(value.get("maxAttempts"), "maxAttempts"),
            parser_request=dict(parser_request),
        )

    def to_payload(self) -> dict[str, Any]:
        return {
            "outboxId": self.outbox_id,
            "tenantId": self.tenant_id,
            "datasetId": self.dataset_id,
            "documentId": self.document_id,
            "parserJobId": self.parser_job_id,
            "attempt": self.attempt,
            "maxAttempts": self.max_attempts,
            "parserRequest": dict(self.parser_request),
        }


@dataclass(frozen=True)
class ParserWorkerOutcome:
    status: str
    ack: bool
    message: ParserJobMessage | None = None
    error: str = ""


def handle_parser_message(
    raw_message: ParserJobMessage | Mapping[str, Any] | bytes | str,
    *,
    redis_client,
    publisher,
    worker_id: str,
    backend_base_url: str,
    backend_token: str = "",
    runner=None,
    mineru_runner=None,
    lease_ttl_seconds: int = 300,
    idempotency_ttl_seconds: int = 86400,
) -> ParserWorkerOutcome:
    message = parse_parser_message(raw_message)
    if not acquire_job_lease(
        redis_client,
        message,
        worker_id=worker_id,
        ttl_seconds=lease_ttl_seconds,
    ):
        return ParserWorkerOutcome(status="lease_skipped", ack=True, message=message)

    try:
        if job_idempotency_exists(redis_client, message):
            return ParserWorkerOutcome(status="duplicate_skipped", ack=True, message=message)

        try:
            result = run_parser_job(
                message,
                backend_base_url=backend_base_url,
                backend_token=backend_token,
                runner=runner,
                mineru_runner=mineru_runner,
            )
        except Exception as error:
            if has_mineru_task(message.parser_request):
                return publish_deferred_retry(
                    publisher,
                    message,
                    last_error=str(error),
                )
            return publish_retry_or_dead(
                publisher,
                message,
                last_error=str(error),
            )

        status = str(result.get("status") or "").strip().lower()
        if status == "succeeded":
            mark_job_idempotency(redis_client, message, ttl_seconds=idempotency_ttl_seconds)
            return ParserWorkerOutcome(status="succeeded", ack=True, message=message)
        if status == "submitted":
            return publish_deferred_retry(
                publisher,
                message,
                parser_request=parser_request_with_mineru_task(message.parser_request, result),
                last_error="parser job deferred",
            )
        if status == "failed":
            return publish_retry_or_dead(
                publisher,
                message,
                last_error=parser_result_error(result) or "parser job failed",
            )
        return publish_retry_or_dead(
            publisher,
            message,
            last_error=f"unknown parser status: {status or '<empty>'}",
        )
    finally:
        release_job_lease(redis_client, message, worker_id=worker_id)


def parse_parser_message(raw_message: ParserJobMessage | Mapping[str, Any] | bytes | str) -> ParserJobMessage:
    if isinstance(raw_message, ParserJobMessage):
        return raw_message
    if isinstance(raw_message, Mapping):
        return ParserJobMessage.from_dict(raw_message)
    if isinstance(raw_message, (bytes, str)):
        return ParserJobMessage.from_json(raw_message)
    raise ValueError("parser message must be JSON bytes, string, mapping, or ParserJobMessage")


def run_parser_job(
    message: ParserJobMessage,
    *,
    backend_base_url: str,
    backend_token: str,
    runner=None,
    mineru_runner=None,
) -> Mapping[str, Any]:
    request = dict(message.parser_request)
    mineru_task = request.get("mineruTask")
    if isinstance(mineru_task, Mapping) and str(mineru_task.get("taskId") or "").strip():
        if mineru_runner is None:
            mineru_runner = default_mineru_runner()
        return mineru_runner(
            request,
            task_id=str(mineru_task["taskId"]).strip(),
            backend_base_url=backend_base_url,
            backend_token=backend_token,
        )

    if runner is None:
        from parser_worker.runner import execute_parse_job

        runner = execute_parse_job
    return runner(
        request,
        backend_base_url=backend_base_url,
        backend_token=backend_token,
    )


def default_mineru_runner():
    from parser_worker.config import load_config
    from parser_worker.mineru_client import MineruClient
    from parser_worker.runner import complete_mineru_parse_job

    config = load_config()
    mineru_client = MineruClient(
        token=config.mineru.token,
        timeout_seconds=config.mineru.timeout_seconds,
    )

    def run(request, *, task_id: str, backend_base_url: str, backend_token: str):
        return complete_mineru_parse_job(
            request,
            task_id=task_id,
            backend_base_url=backend_base_url,
            backend_token=backend_token,
            mineru_client=mineru_client,
        )

    return run


def publish_retry_or_dead(
    publisher,
    message: ParserJobMessage,
    *,
    last_error: str,
    parser_request: Mapping[str, Any] | None = None,
) -> ParserWorkerOutcome:
    if message.attempt < message.max_attempts:
        retry = next_attempt_message(
            message,
            parser_request=parser_request,
            last_error=last_error,
        )
        try:
            publisher.publish_retry(retry)
        except Exception as error:
            return ParserWorkerOutcome(status="publish_failed", ack=False, message=message, error=str(error))
        return ParserWorkerOutcome(status="retry_published", ack=True, message=retry, error=last_error)

    dead = message_with_request(
        message,
        parser_request=parser_request,
        last_error=last_error,
    )
    try:
        publisher.publish_dead(dead)
    except Exception as error:
        return ParserWorkerOutcome(status="publish_failed", ack=False, message=message, error=str(error))
    return ParserWorkerOutcome(status="dead_published", ack=True, message=dead, error=last_error)


def publish_deferred_retry(
    publisher,
    message: ParserJobMessage,
    *,
    last_error: str,
    parser_request: Mapping[str, Any] | None = None,
) -> ParserWorkerOutcome:
    retry = next_attempt_message(
        message,
        parser_request=parser_request,
        last_error=last_error,
    )
    try:
        publisher.publish_retry(retry)
    except Exception as error:
        return ParserWorkerOutcome(status="publish_failed", ack=False, message=message, error=str(error))
    return ParserWorkerOutcome(status="retry_published", ack=True, message=retry, error=last_error)


def next_attempt_message(
    message: ParserJobMessage,
    *,
    parser_request: Mapping[str, Any] | None = None,
    last_error: str,
) -> ParserJobMessage:
    updated = message_with_request(
        message,
        parser_request=parser_request,
        last_error=last_error,
    )
    return ParserJobMessage(
        outbox_id=updated.outbox_id,
        tenant_id=updated.tenant_id,
        dataset_id=updated.dataset_id,
        document_id=updated.document_id,
        parser_job_id=updated.parser_job_id,
        attempt=updated.attempt + 1,
        max_attempts=updated.max_attempts,
        parser_request=updated.parser_request,
    )


def message_with_request(
    message: ParserJobMessage,
    *,
    parser_request: Mapping[str, Any] | None = None,
    last_error: str,
) -> ParserJobMessage:
    request = dict(parser_request or message.parser_request)
    if last_error:
        request["lastError"] = last_error
    return ParserJobMessage(
        outbox_id=message.outbox_id,
        tenant_id=message.tenant_id,
        dataset_id=message.dataset_id,
        document_id=message.document_id,
        parser_job_id=message.parser_job_id,
        attempt=message.attempt,
        max_attempts=message.max_attempts,
        parser_request=request,
    )


def parser_request_with_mineru_task(
    parser_request: Mapping[str, Any],
    parser_result: Mapping[str, Any],
) -> Mapping[str, Any]:
    request = dict(parser_request)
    result = parser_result.get("parserResult")
    mineru_task = None
    if isinstance(result, Mapping) and isinstance(result.get("mineruTask"), Mapping):
        mineru_task = result["mineruTask"]
    elif isinstance(parser_result.get("mineruTask"), Mapping):
        mineru_task = parser_result["mineruTask"]
    if mineru_task is not None:
        request["mineruTask"] = dict(mineru_task)
    return request


def has_mineru_task(parser_request: Mapping[str, Any]) -> bool:
    mineru_task = parser_request.get("mineruTask")
    return isinstance(mineru_task, Mapping) and bool(str(mineru_task.get("taskId") or "").strip())


def parser_result_error(parser_result: Mapping[str, Any]) -> str:
    for value in (parser_result.get("error"), parser_result.get("parserResult")):
        if isinstance(value, Mapping):
            nested_error = value.get("error")
            if isinstance(nested_error, Mapping):
                message = str(nested_error.get("message") or "").strip()
                if message:
                    return message
            message = str(value.get("message") or "").strip()
            if message:
                return message
        elif value:
            return str(value)
    return ""


class RabbitMqParserPublisher:
    def __init__(self, channel, config: ParserWorkerConfig, properties_factory=None) -> None:
        self._channel = channel
        self._config = config
        if properties_factory is None:
            import pika

            properties_factory = lambda: pika.BasicProperties(
                content_type="application/json",
                delivery_mode=2,
            )
        self._properties_factory = properties_factory

    def publish_retry(self, message: ParserJobMessage) -> None:
        self._publish(self._config.retry_routing_key, message)

    def publish_dead(self, message: ParserJobMessage) -> None:
        self._publish(self._config.dead_routing_key, message)

    def _publish(self, routing_key: str, message: ParserJobMessage) -> None:
        self._channel.basic_publish(
            exchange=self._config.exchange,
            routing_key=routing_key,
            body=json_dumps(message.to_payload()).encode("utf-8"),
            properties=self._properties_factory(),
        )


def run_worker(config: ParserWorkerConfig | None = None) -> None:
    config = config or ParserWorkerConfig.from_env()

    import pika
    import redis

    rabbitmq = pika.BlockingConnection(pika.URLParameters(config.rabbitmq_url))
    channel = rabbitmq.channel()
    declare_parser_topology(channel, config)
    channel.basic_qos(prefetch_count=config.prefetch_count)

    redis_client = redis.Redis.from_url(config.redis_url, decode_responses=True)
    publisher = RabbitMqParserPublisher(channel, config)

    def on_message(ch, method, _properties, body):
        outcome = handle_parser_message(
            body,
            redis_client=redis_client,
            publisher=publisher,
            worker_id=config.worker_id,
            backend_base_url=config.backend_base_url,
            backend_token=config.backend_token,
            lease_ttl_seconds=config.lease_ttl_seconds,
            idempotency_ttl_seconds=config.idempotency_ttl_seconds,
        )
        if outcome.ack:
            ch.basic_ack(delivery_tag=method.delivery_tag)
        else:
            ch.basic_nack(delivery_tag=method.delivery_tag, requeue=True)

    channel.basic_consume(
        queue=config.execute_queue,
        on_message_callback=on_message,
        auto_ack=False,
    )
    channel.start_consuming()


def declare_parser_topology(channel, config: ParserWorkerConfig) -> None:
    channel.exchange_declare(
        exchange=config.exchange,
        exchange_type="direct",
        durable=True,
    )
    channel.queue_declare(queue=config.execute_queue, durable=True)
    channel.queue_bind(
        queue=config.execute_queue,
        exchange=config.exchange,
        routing_key=config.execute_routing_key,
    )
    channel.queue_declare(
        queue=config.retry_queue,
        durable=True,
        arguments={
            "x-message-ttl": config.retry_ttl_ms,
            "x-dead-letter-exchange": config.exchange,
            "x-dead-letter-routing-key": config.execute_routing_key,
        },
    )
    channel.queue_bind(
        queue=config.retry_queue,
        exchange=config.exchange,
        routing_key=config.retry_routing_key,
    )
    channel.queue_declare(queue=config.dead_queue, durable=True)
    channel.queue_bind(
        queue=config.dead_queue,
        exchange=config.exchange,
        routing_key=config.dead_routing_key,
    )


def acquire_job_lease(redis_client, message: ParserJobMessage, *, worker_id: str, ttl_seconds: int) -> bool:
    return bool(
        redis_client.set(
            parser_lease_key(message),
            worker_id,
            nx=True,
            ex=positive_int(ttl_seconds, "ttl_seconds"),
        )
    )


def release_job_lease(redis_client, message: ParserJobMessage, *, worker_id: str) -> bool:
    key = parser_lease_key(message)
    current_value = redis_client.get(key)
    if not same_redis_value(current_value, worker_id):
        return False
    return bool(redis_client.delete(key))


def mark_job_idempotency(redis_client, message: ParserJobMessage, *, ttl_seconds: int) -> bool:
    return bool(
        redis_client.set(
            parser_idempotency_key(message),
            "1",
            nx=True,
            ex=positive_int(ttl_seconds, "ttl_seconds"),
        )
    )


def job_idempotency_exists(redis_client, message: ParserJobMessage) -> bool:
    return redis_client.get(parser_idempotency_key(message)) is not None


def parser_lease_key(message: ParserJobMessage) -> str:
    return f"{LEASE_KEY_PREFIX}:{message.parser_job_id}"


def parser_idempotency_key(message: ParserJobMessage) -> str:
    source = message.parser_request.get("source")
    source_hash = ""
    if isinstance(source, Mapping):
        source_hash = str(source.get("sourceHash") or "").strip()
    suffix = source_hash or f"document-{message.document_id}"
    return f"{IDEMPOTENCY_KEY_PREFIX}:{message.parser_job_id}:{suffix}"


def same_redis_value(value: Any, expected: str) -> bool:
    if isinstance(value, bytes):
        value = value.decode("utf-8")
    return str(value) == expected


def positive_int(value: Any, name: str) -> int:
    try:
        parsed = int(value)
    except (TypeError, ValueError) as error:
        raise ValueError(f"{name} must be a positive integer") from error
    if parsed <= 0:
        raise ValueError(f"{name} must be a positive integer")
    return parsed


def parse_positive_int(value: str | None, default: int) -> int:
    if value is None or not str(value).strip():
        return default
    return positive_int(value, "value")


def non_empty(value: str | None, default: str) -> str:
    if value is None:
        return default
    normalized = str(value).strip()
    return normalized or default


def mask_secret(value: str) -> str:
    if not value:
        return ""
    if len(value) <= 4:
        return "***"
    if len(value) <= 8:
        return f"{value[:2]}****{value[-2:]}"
    return f"{value[:4]}****{value[-4:]}"


def json_dumps(value: Mapping[str, Any]) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True)


def main() -> None:
    run_worker()


if __name__ == "__main__":
    main()
