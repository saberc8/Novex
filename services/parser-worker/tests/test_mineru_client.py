import unittest

from parser_worker.mineru_client import MineruClient, MineruError, default_ssl_context


class FakeTransport:
    def __init__(self, responses):
        self.responses = list(responses)
        self.requests = []

    def __call__(self, request):
        self.requests.append(request)
        return self.responses.pop(0)


class MineruClientTest(unittest.TestCase):
    def test_create_extract_task_sends_authorized_v4_request(self):
        transport = FakeTransport(
            [
                {
                    "code": 0,
                    "msg": "ok",
                    "trace_id": "trace-1",
                    "data": {
                        "task_id": "task-1",
                        "state": "pending",
                    },
                }
            ]
        )
        client = MineruClient(token="token-123", transport=transport)

        task = client.create_extract_task(
            "https://cdn-mineru.openxlab.org.cn/demo/example.pdf",
            model_version="vlm",
            is_ocr=True,
        )

        self.assertEqual(task.task_id, "task-1")
        self.assertEqual(task.state, "pending")
        request = transport.requests[0]
        self.assertEqual(request.method, "POST")
        self.assertEqual(request.path, "/api/v4/extract/task")
        self.assertEqual(request.headers["Authorization"], "Bearer token-123")
        self.assertEqual(
            request.json,
            {
                "url": "https://cdn-mineru.openxlab.org.cn/demo/example.pdf",
                "model_version": "vlm",
                "is_ocr": True,
                "enable_formula": True,
                "enable_table": True,
                "language": "ch",
            },
        )

    def test_create_extract_task_defaults_state_when_api_omits_it(self):
        transport = FakeTransport(
            [
                {
                    "code": 0,
                    "msg": "ok",
                    "trace_id": "trace-1",
                    "data": {
                        "task_id": "task-1",
                    },
                }
            ]
        )
        client = MineruClient(token="token-123", transport=transport)

        task = client.create_extract_task("https://cdn-mineru.openxlab.org.cn/demo/example.pdf")

        self.assertEqual(task.task_id, "task-1")
        self.assertEqual(task.state, "submitted")

    def test_get_extract_task_maps_done_result(self):
        transport = FakeTransport(
            [
                {
                    "code": 0,
                    "msg": "ok",
                    "trace_id": "trace-2",
                    "data": {
                        "task_id": "task-2",
                        "state": "done",
                        "full_zip_url": "https://cdn-mineru.openxlab.org.cn/pdf/result.zip",
                        "err_msg": "",
                    },
                }
            ]
        )
        client = MineruClient(token="token-123", transport=transport)

        task = client.get_extract_task("task-2")

        self.assertEqual(task.task_id, "task-2")
        self.assertEqual(task.state, "done")
        self.assertEqual(task.full_zip_url, "https://cdn-mineru.openxlab.org.cn/pdf/result.zip")
        self.assertEqual(transport.requests[0].method, "GET")
        self.assertEqual(transport.requests[0].path, "/api/v4/extract/task/task-2")

    def test_api_errors_do_not_include_raw_token(self):
        transport = FakeTransport(
            [
                {
                    "code": 1001,
                    "msg": "invalid token",
                    "trace_id": "trace-3",
                    "data": None,
                }
            ]
        )
        client = MineruClient(token="secret-token-value", transport=transport)

        with self.assertRaises(MineruError) as error:
            client.create_extract_task("https://example.com/file.pdf")

        self.assertIn("invalid token", str(error.exception))
        self.assertNotIn("secret-token-value", str(error.exception))

    def test_default_ssl_context_is_available_for_live_transport(self):
        self.assertIsNotNone(default_ssl_context())


if __name__ == "__main__":
    unittest.main()
