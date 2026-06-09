import inspect
import unittest
from io import BytesIO
from zipfile import ZipFile

from parser_worker.mineru_client import MineruTask
from parser_worker.runner import (
    complete_mineru_parse_job,
    default_zip_fetcher,
    execute_parse_job,
    extract_mineru_markdown_from_zip,
)


class ParserWorkerRunnerTest(unittest.TestCase):
    def test_execute_inline_markdown_posts_succeeded_result_to_backend(self):
        poster = FakePoster()
        result = execute_parse_job(
            {
                "tenantId": 1,
                "datasetId": 7,
                "documentId": 42,
                "parserJobId": 99,
                "source": {
                    "kind": "inlineText",
                    "contentType": "text/markdown",
                    "name": "handbook.md",
                    "content": "# 入职培训\n第一天完成安全培训。",
                    "sourceHash": "abc123",
                },
                "options": {"maxChunkChars": 120, "chunkOverlapChars": 0},
            },
            backend_base_url="http://backend.local",
            backend_token="token-1",
            http_post=poster,
        )

        self.assertEqual(result["status"], "succeeded")
        self.assertEqual(result["callbackStatus"], "posted")
        self.assertEqual(
            poster.calls[0]["url"],
            "http://backend.local/ai/knowledge/datasets/7/documents/parsed",
        )
        self.assertEqual(poster.calls[0]["headers"]["Authorization"], "Bearer token-1")
        payload = poster.calls[0]["json"]
        self.assertEqual(payload["name"], "handbook.md")
        self.assertEqual(payload["contentType"], "text/markdown")
        self.assertEqual(payload["parserResult"]["status"], "succeeded")
        self.assertEqual(payload["parserResult"]["parserJobId"], 99)
        self.assertGreater(len(payload["parserResult"]["chunks"]), 0)

    def test_execute_inline_markdown_rejects_failed_backend_envelope(self):
        poster = FakePoster(response={"statusCode": 200, "body": {"code": "401", "success": False, "msg": "未授权"}})

        with self.assertRaisesRegex(RuntimeError, "backend callback failed"):
            execute_parse_job(
                {
                    "tenantId": 1,
                    "datasetId": 7,
                    "documentId": 42,
                    "parserJobId": 99,
                    "source": {
                        "kind": "inlineText",
                        "contentType": "text/markdown",
                        "name": "handbook.md",
                        "content": "# 入职培训\n第一天完成安全培训。",
                        "sourceHash": "abc123",
                    },
                },
                backend_base_url="http://backend.local",
                backend_token="token-1",
                http_post=poster,
            )

    def test_execute_relative_object_storage_text_fetches_source_before_parse(self):
        poster = FakePoster()
        fetcher = FakeFetcher({"http://backend.local/file/knowledge/88.md": "# 手册\n按时培训。"})

        result = execute_parse_job(
            {
                "tenantId": 1,
                "datasetId": 7,
                "documentId": 42,
                "parserJobId": 99,
                "source": {
                    "kind": "objectStorage",
                    "contentType": "text/markdown",
                    "name": "handbook.md",
                    "uri": "/file/knowledge/88.md",
                    "fileId": 88,
                    "sourceHash": "abc123",
                },
            },
            backend_base_url="http://backend.local/",
            backend_token="token-1",
            source_fetcher=fetcher,
            http_post=poster,
        )

        self.assertEqual(result["status"], "succeeded")
        self.assertEqual(result["preparedRequest"]["source"]["kind"], "inlineText")
        self.assertEqual(result["preparedRequest"]["source"]["uri"], "http://backend.local/file/knowledge/88.md")
        self.assertEqual(fetcher.uris, ["http://backend.local/file/knowledge/88.md"])
        self.assertIn("按时培训", poster.calls[0]["json"]["parserResult"]["chunks"][0]["text"])

    def test_execute_submitted_mineru_task_posts_status_without_completed_ingestion(self):
        poster = FakePoster()
        mineru = FakeMineruClient()

        result = execute_parse_job(
            {
                "tenantId": 1,
                "datasetId": 7,
                "documentId": 42,
                "parserJobId": 99,
                "source": {
                    "kind": "remoteUrl",
                    "contentType": "application/pdf",
                    "name": "handbook.pdf",
                    "uri": "https://objects.example.com/handbook.pdf",
                },
            },
            backend_base_url="http://backend.local",
            backend_token="token-1",
            mineru_client=mineru,
            http_post=poster,
        )

        self.assertEqual(result["status"], "submitted")
        self.assertEqual(result["callbackStatus"], "deferred")
        self.assertEqual(
            poster.calls[0]["url"],
            "http://backend.local/ai/knowledge/datasets/7/parse-jobs/99/status",
        )
        self.assertEqual(poster.calls[0]["headers"]["Authorization"], "Bearer token-1")
        self.assertEqual(poster.calls[0]["json"]["status"], "submitted")
        self.assertEqual(poster.calls[0]["json"]["callbackStatus"], "deferred")
        self.assertEqual(
            poster.calls[0]["json"]["parserResult"]["mineruTask"]["taskId"],
            "task-1",
        )
        self.assertEqual(mineru.created_urls, ["https://objects.example.com/handbook.pdf"])

    def test_extract_mineru_markdown_prefers_auto_full_md_from_zip(self):
        zip_bytes = build_zip_bytes(
            {
                "images/ignored.txt": "not markdown",
                "nested/auto.md": "# Wrong\n",
                "result/auto_full.md": "# 薪酬政策\n[[page: 5]]\n正文",
            }
        )

        markdown = extract_mineru_markdown_from_zip(zip_bytes)

        self.assertIn("# 薪酬政策", markdown)
        self.assertIn("[[page: 5]]", markdown)

    def test_default_zip_fetcher_uses_mineru_ssl_context(self):
        source = inspect.getsource(default_zip_fetcher)

        self.assertIn("default_ssl_context()", source)
        self.assertIn("curl", source)

    def test_complete_done_mineru_task_downloads_zip_and_posts_parsed_document(self):
        poster = FakePoster()
        mineru = FakeMineruClient(
            completed=MineruTask(
                task_id="task-1",
                state="done",
                full_zip_url="https://cdn.example.com/result.zip",
            )
        )
        zip_fetcher = FakeZipFetcher(
            {
                "https://cdn.example.com/result.zip": build_zip_bytes(
                    {"auto_full.md": "# 薪酬政策\n[[page: 5]]\n| 岗位 | 补贴 |\n| --- | --- |\n| 工程师 | 100 |\n"}
                )
            }
        )

        result = complete_mineru_parse_job(
            {
                "tenantId": 1,
                "datasetId": 7,
                "documentId": 42,
                "parserJobId": 99,
                "source": {
                    "kind": "remoteUrl",
                    "contentType": "application/pdf",
                    "name": "salary-policy.pdf",
                    "uri": "https://objects.example.com/salary-policy.pdf",
                },
            },
            task_id="task-1",
            backend_base_url="http://backend.local",
            backend_token="token-1",
            mineru_client=mineru,
            zip_fetcher=zip_fetcher,
            http_post=poster,
        )

        self.assertEqual(result["status"], "succeeded")
        self.assertEqual(result["callbackStatus"], "posted")
        self.assertEqual(mineru.polled_task_ids, ["task-1"])
        self.assertEqual(zip_fetcher.urls, ["https://cdn.example.com/result.zip"])
        payload = poster.calls[0]["json"]
        self.assertEqual(payload["name"], "salary-policy.pdf")
        self.assertEqual(payload["parserResult"]["metadata"]["parser"], "mineru")
        self.assertEqual(payload["parserResult"]["metadata"]["mineru"]["taskId"], "task-1")
        self.assertEqual(payload["parserResult"]["parserJobId"], 99)
        self.assertGreater(len(payload["parserResult"]["chunks"]), 0)

    def test_complete_pending_mineru_task_posts_status_without_completed_ingestion(self):
        poster = FakePoster()
        mineru = FakeMineruClient(completed=MineruTask(task_id="task-2", state="pending"))

        result = complete_mineru_parse_job(
            {
                "tenantId": 1,
                "datasetId": 7,
                "documentId": 42,
                "parserJobId": 99,
                "source": {
                    "kind": "remoteUrl",
                    "contentType": "application/pdf",
                    "name": "handbook.pdf",
                    "uri": "https://objects.example.com/handbook.pdf",
                },
            },
            task_id="task-2",
            backend_base_url="http://backend.local",
            backend_token="token-1",
            mineru_client=mineru,
            http_post=poster,
        )

        self.assertEqual(result["status"], "submitted")
        self.assertEqual(result["callbackStatus"], "deferred")
        self.assertEqual(
            poster.calls[0]["url"],
            "http://backend.local/ai/knowledge/datasets/7/parse-jobs/99/status",
        )
        self.assertEqual(poster.calls[0]["json"]["status"], "submitted")
        self.assertEqual(
            poster.calls[0]["json"]["mineruTask"]["taskId"],
            "task-2",
        )


class FakePoster:
    def __init__(self, response=None):
        self.calls = []
        self.response = response or {"statusCode": 200, "body": {"code": "200", "success": True}}

    def __call__(self, url, *, headers, json):
        self.calls.append({"url": url, "headers": dict(headers), "json": json})
        return self.response


class FakeFetcher:
    def __init__(self, content_by_uri):
        self.content_by_uri = content_by_uri
        self.uris = []

    def __call__(self, uri, source):
        self.uris.append(uri)
        return self.content_by_uri[uri]


class FakeMineruClient:
    def __init__(self, completed=None):
        self.created_urls = []
        self.completed = completed
        self.polled_task_ids = []

    def create_extract_task(self, source_url, **_kwargs):
        self.created_urls.append(source_url)
        return MineruTask(task_id="task-1", state="pending", full_zip_url="")

    def get_extract_task(self, task_id):
        self.polled_task_ids.append(task_id)
        return self.completed


class FakeZipFetcher:
    def __init__(self, bytes_by_url):
        self.bytes_by_url = bytes_by_url
        self.urls = []

    def __call__(self, url):
        self.urls.append(url)
        return self.bytes_by_url[url]


def build_zip_bytes(files):
    buffer = BytesIO()
    with ZipFile(buffer, "w") as archive:
        for name, content in files.items():
            archive.writestr(name, content)
    return buffer.getvalue()


if __name__ == "__main__":
    unittest.main()
