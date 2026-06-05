import unittest

from parser_worker.mineru_client import MineruTask
from parser_worker.runner import execute_parse_job


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

    def test_execute_submitted_mineru_task_does_not_post_completed_ingestion(self):
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
        self.assertEqual(poster.calls, [])
        self.assertEqual(mineru.created_urls, ["https://objects.example.com/handbook.pdf"])


class FakePoster:
    def __init__(self):
        self.calls = []

    def __call__(self, url, *, headers, json):
        self.calls.append({"url": url, "headers": dict(headers), "json": json})
        return {"statusCode": 200, "body": {"code": "200", "success": True}}


class FakeFetcher:
    def __init__(self, content_by_uri):
        self.content_by_uri = content_by_uri
        self.uris = []

    def __call__(self, uri, source):
        self.uris.append(uri)
        return self.content_by_uri[uri]


class FakeMineruClient:
    def __init__(self):
        self.created_urls = []

    def create_extract_task(self, source_url, **_kwargs):
        self.created_urls.append(source_url)
        return MineruTask(task_id="task-1", state="pending", full_zip_url="")


if __name__ == "__main__":
    unittest.main()
