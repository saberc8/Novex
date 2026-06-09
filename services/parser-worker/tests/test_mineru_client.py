import unittest
from pathlib import Path

from parser_worker.mineru_client import MineruClient, MineruError, default_ssl_context


class FakeTransport:
    def __init__(self, responses):
        self.responses = list(responses)
        self.requests = []

    def __call__(self, request):
        self.requests.append(request)
        return self.responses.pop(0)


class RoutingTransport:
    def __init__(self):
        self.requests = []

    def __call__(self, request):
        self.requests.append(request)
        if request.method == "POST" and request.path == "/api/v4/file-urls/batch":
            return {
                "code": 0,
                "msg": "ok",
                "data": {
                    "batch_id": "batch-1",
                    "file_urls": ["https://upload.example.com/demo.pdf?sig=1"],
                },
            }
        if request.method == "PUT" and request.base_url.startswith("https://upload.example.com/"):
            return {}
        if request.method == "GET" and request.path == "/api/v4/extract-results/batch/batch-1":
            return {
                "code": 0,
                "msg": "ok",
                "data": {
                    "batch_id": "batch-1",
                    "extract_result": [
                        {
                            "file_name": "demo.pdf",
                            "state": "done",
                            "full_zip_url": "https://cdn-mineru.openxlab.org.cn/pdf/result.zip",
                            "err_msg": "",
                            "data_id": "job-1",
                        }
                    ],
                },
            }
        raise AssertionError(f"unexpected request: {request}")


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

    def test_localhost_source_uses_v4_batch_file_upload(self):
        transport = RoutingTransport()
        client = MineruClient(
            token="token-123",
            transport=transport,
            source_reader=lambda url: b"%PDF-1.7",
        )

        task = client.create_extract_task(
            "http://127.0.0.1:4398/file/knowledge/1.pdf",
            file_name="demo.pdf",
            data_id="job-1",
            model_version="vlm",
            is_ocr=True,
            enable_formula=True,
            enable_table=True,
            language="ch",
        )

        self.assertEqual(task.task_id, "batch:batch-1")
        self.assertEqual(task.state, "submitted")
        apply_request = transport.requests[0]
        self.assertEqual(apply_request.method, "POST")
        self.assertEqual(apply_request.path, "/api/v4/file-urls/batch")
        self.assertEqual(apply_request.json["files"][0]["name"], "demo.pdf")
        self.assertEqual(apply_request.json["files"][0]["data_id"], "job-1")
        self.assertTrue(apply_request.json["files"][0]["is_ocr"])
        upload_request = transport.requests[1]
        self.assertEqual(upload_request.method, "PUT")
        self.assertEqual(upload_request.body, b"%PDF-1.7")

    def test_batch_task_id_polls_batch_extract_results(self):
        transport = RoutingTransport()
        client = MineruClient(token="token-123", transport=transport)

        task = client.get_extract_task("batch:batch-1")

        self.assertEqual(task.task_id, "batch:batch-1")
        self.assertEqual(task.state, "done")
        self.assertEqual(task.full_zip_url, "https://cdn-mineru.openxlab.org.cn/pdf/result.zip")
        self.assertEqual(task.raw["batch_id"], "batch-1")
        self.assertEqual(task.raw["data_id"], "job-1")

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

    def test_certifi_is_declared_as_worker_dependency(self):
        requirements = (
            Path(__file__).resolve().parents[1] / "requirements.txt"
        ).read_text(encoding="utf-8")

        self.assertIn("certifi", requirements)


if __name__ == "__main__":
    unittest.main()
