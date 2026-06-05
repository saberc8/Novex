import unittest

from parser_worker.mineru_client import MineruTask
from parser_worker.parse import parse_local_request, parse_mineru_markdown_result, parse_request


class ParserWorkerParseTest(unittest.TestCase):
    def test_inline_markdown_parse_emits_layout_blocks_and_search_chunks(self):
        result = parse_local_request(
            {
                "tenantId": 1,
                "datasetId": 7,
                "documentId": 42,
                "parserJobId": 99,
                "source": {
                    "kind": "inlineText",
                    "contentType": "text/markdown",
                    "name": "handbook.md",
                    "content": "# 入职培训\n[[page: 3]]\n第一天需要完成安全培训。\n\n[[image: key=img/search-flow.png bbox=10,20,300,180 caption=流程图显示 hybrid recall 和 rerank 链路]]",
                    "sourceHash": "abc123",
                },
                "options": {
                    "maxChunkChars": 120,
                    "chunkOverlapChars": 0,
                },
            }
        )

        self.assertEqual(result["status"], "succeeded")
        self.assertEqual(result["metadata"]["parser"], "novex-parser-worker-local")
        self.assertEqual(result["metadata"]["sourceHash"], "abc123")
        self.assertEqual(result["tenantId"], 1)
        self.assertEqual(result["datasetId"], 7)
        self.assertEqual(result["documentId"], 42)
        self.assertEqual(result["parserJobId"], 99)

        title_block = result["blocks"][0]
        paragraph_block = result["blocks"][1]
        image_block = result["blocks"][2]
        self.assertEqual(title_block["type"], "title")
        self.assertEqual(title_block["sectionPath"], ["入职培训"])
        self.assertEqual(paragraph_block["type"], "paragraph")
        self.assertEqual(paragraph_block["pageNo"], 3)
        self.assertEqual(paragraph_block["sectionPath"], ["入职培训"])
        self.assertEqual(image_block["type"], "image")
        self.assertEqual(image_block["bbox"]["width"], 300)

        text_chunk = result["chunks"][0]
        image_chunk = result["chunks"][1]
        self.assertEqual(text_chunk["chunkUid"], "42:0")
        self.assertEqual(text_chunk["segmentType"], "text")
        self.assertEqual(text_chunk["citation"]["blockIds"], [paragraph_block["blockId"]])
        self.assertIn("handbook.md", text_chunk["semanticSearchText"])
        self.assertIn("入职培训", text_chunk["semanticSearchText"])
        self.assertIn("安全培训", text_chunk["semanticSearchText"])
        self.assertGreater(text_chunk["tokenCount"], 0)

        self.assertEqual(image_chunk["segmentType"], "image")
        self.assertEqual(image_chunk["imageAccessKeys"], ["img/search-flow.png"])
        self.assertEqual(image_chunk["displayCapability"], "precise_anchor")
        self.assertEqual(image_chunk["citation"]["pageNo"], 3)
        self.assertEqual(image_chunk["citation"]["blockIds"], [image_block["blockId"]])
        self.assertIn("hybrid recall", image_chunk["semanticSearchText"])

    def test_inline_csv_parse_keeps_table_header_in_chunks(self):
        result = parse_local_request(
            {
                "tenantId": 1,
                "datasetId": 7,
                "documentId": 43,
                "parserJobId": 100,
                "source": {
                    "kind": "inlineText",
                    "contentType": "text/csv",
                    "name": "training.csv",
                    "content": "employee,deadline,status\nAlice,Friday,done\nBob,Monday,pending\nCharlie,Wednesday,pending",
                },
                "options": {
                    "maxChunkChars": 48,
                    "chunkOverlapChars": 0,
                },
            }
        )

        self.assertEqual(result["status"], "succeeded")
        self.assertEqual(result["blocks"][0]["type"], "table")
        self.assertEqual(result["chunks"][0]["segmentType"], "table")
        self.assertEqual(result["chunks"][0]["tableHeader"], ["employee", "deadline", "status"])
        self.assertTrue(
            all(chunk["text"].startswith("employee,deadline,status\n") for chunk in result["chunks"])
        )
        self.assertTrue(
            all("employee deadline status" in chunk["semanticSearchText"] for chunk in result["chunks"])
        )

    def test_default_parse_keeps_csv_native_instead_of_submitting_to_mineru(self):
        converter = FakePdfConverter(
            {
                "uri": "https://objects.example.com/converted/should-not-use.pdf",
                "name": "should-not-use.pdf",
            }
        )
        mineru = FakeMineruClient()

        result = parse_request(
            {
                "tenantId": 1,
                "datasetId": 7,
                "documentId": 46,
                "parserJobId": 103,
                "source": {
                    "kind": "inlineText",
                    "contentType": "text/csv",
                    "name": "training.csv",
                    "content": "employee,deadline,status\nAlice,Friday,done\nBob,Monday,pending",
                },
            },
            pdf_converter=converter,
            mineru_client=mineru,
        )

        self.assertEqual(result["status"], "succeeded")
        self.assertEqual(result["metadata"]["strategy"], "native_structured")
        self.assertEqual(result["chunks"][0]["segmentType"], "table")
        self.assertEqual(converter.sources, [])
        self.assertEqual(mineru.created_urls, [])

    def test_default_parse_extracts_xlsx_tables_without_pdf_conversion(self):
        converter = FakePdfConverter(
            {
                "uri": "https://objects.example.com/converted/should-not-use.pdf",
                "name": "should-not-use.pdf",
            }
        )
        mineru = FakeMineruClient()
        table_extractor = FakeTableExtractor(
            {
                "content": "employee,deadline,status\nAlice,Friday,done\nBob,Monday,pending",
                "contentType": "text/csv",
                "name": "training.csv",
                "sourceHash": "xlsx-table-hash",
            }
        )

        result = parse_request(
            {
                "tenantId": 1,
                "datasetId": 7,
                "documentId": 48,
                "parserJobId": 105,
                "source": {
                    "kind": "objectStorage",
                    "contentType": "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                    "name": "training.xlsx",
                    "uri": "https://objects.example.com/raw/training.xlsx",
                },
            },
            pdf_converter=converter,
            mineru_client=mineru,
            table_extractor=table_extractor,
        )

        self.assertEqual(result["status"], "succeeded")
        self.assertEqual(result["metadata"]["strategy"], "native_structured")
        self.assertEqual(result["metadata"]["sourceHash"], "xlsx-table-hash")
        self.assertEqual(result["chunks"][0]["segmentType"], "table")
        self.assertEqual(table_extractor.sources[0]["name"], "training.xlsx")
        self.assertEqual(converter.sources, [])
        self.assertEqual(mineru.created_urls, [])

    def test_default_parse_converts_office_source_before_submitting_to_mineru(self):
        converter = FakePdfConverter(
            {
                "uri": "https://objects.example.com/converted/training.pdf",
                "name": "training.pdf",
                "sourceHash": "converted-hash",
            }
        )
        mineru = FakeMineruClient()

        result = parse_request(
            {
                "tenantId": 1,
                "datasetId": 7,
                "documentId": 44,
                "parserJobId": 101,
                "source": {
                    "kind": "objectStorage",
                    "contentType": "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                    "name": "training.docx",
                    "uri": "https://objects.example.com/raw/training.docx",
                },
                "options": {"ocr": True, "extractTables": True},
            },
            pdf_converter=converter,
            mineru_client=mineru,
        )

        self.assertEqual(result["status"], "submitted")
        self.assertEqual(result["metadata"]["parser"], "mineru")
        self.assertTrue(result["metadata"]["pdfFirst"])
        self.assertEqual(result["normalizedSource"]["uri"], "https://objects.example.com/converted/training.pdf")
        self.assertTrue(result["normalizedSource"]["converted"])
        self.assertEqual(converter.sources[0]["name"], "training.docx")
        self.assertEqual(mineru.created_urls, ["https://objects.example.com/converted/training.pdf"])
        self.assertTrue(mineru.created_options[0]["is_ocr"])
        self.assertTrue(mineru.created_options[0]["enable_table"])
        self.assertEqual(result["mineruTask"]["taskId"], "task-1")

    def test_default_parse_submits_pdf_source_without_conversion(self):
        converter = FakePdfConverter(
            {
                "uri": "https://objects.example.com/converted/should-not-use.pdf",
                "name": "should-not-use.pdf",
            }
        )
        mineru = FakeMineruClient()

        result = parse_request(
            {
                "tenantId": 1,
                "datasetId": 7,
                "documentId": 45,
                "parserJobId": 102,
                "source": {
                    "kind": "remoteUrl",
                    "contentType": "application/pdf",
                    "name": "policy.pdf",
                    "uri": "https://objects.example.com/raw/policy.pdf",
                },
            },
            pdf_converter=converter,
            mineru_client=mineru,
        )

        self.assertEqual(result["status"], "submitted")
        self.assertEqual(result["normalizedSource"]["uri"], "https://objects.example.com/raw/policy.pdf")
        self.assertFalse(result["normalizedSource"]["converted"])
        self.assertEqual(converter.sources, [])
        self.assertEqual(mineru.created_urls, ["https://objects.example.com/raw/policy.pdf"])

    def test_mineru_markdown_result_is_chunked_into_backend_contract(self):
        result = parse_mineru_markdown_result(
            {
                "tenantId": 1,
                "datasetId": 7,
                "documentId": 47,
                "parserJobId": 104,
                "source": {
                    "kind": "remoteUrl",
                    "contentType": "application/pdf",
                    "name": "salary-policy.pdf",
                    "uri": "https://objects.example.com/raw/salary-policy.pdf",
                },
            },
            "# 薪酬政策\n[[page: 5]]\n| 岗位 | 补贴 |\n| --- | --- |\n| 工程师 | 100 |\n",
            mineru_metadata={"pageCount": 8, "sourceHash": "mineru-hash"},
        )

        self.assertEqual(result["status"], "succeeded")
        self.assertEqual(result["metadata"]["parser"], "mineru")
        self.assertEqual(result["metadata"]["strategy"], "mineru_layout")
        self.assertEqual(result["metadata"]["pageCount"], 8)
        self.assertEqual(result["metadata"]["sourceHash"], "mineru-hash")
        self.assertEqual(result["blocks"][1]["type"], "table")
        self.assertEqual(result["chunks"][0]["segmentType"], "table")
        self.assertEqual(result["chunks"][0]["tableHeader"], ["岗位", "补贴"])
        self.assertEqual(result["chunks"][0]["citation"]["pageNo"], 5)
        self.assertIn("salary-policy.pdf", result["chunks"][0]["semanticSearchText"])
        self.assertIn("岗位 补贴", result["chunks"][0]["semanticSearchText"])


class FakePdfConverter:
    def __init__(self, artifact):
        self.artifact = artifact
        self.sources = []

    def __call__(self, source, options):
        self.sources.append(dict(source))
        return self.artifact


class FakeMineruClient:
    def __init__(self):
        self.created_urls = []
        self.created_options = []

    def create_extract_task(self, source_url, **kwargs):
        self.created_urls.append(source_url)
        self.created_options.append(kwargs)
        return MineruTask(task_id="task-1", state="pending", full_zip_url="")


class FakeTableExtractor:
    def __init__(self, artifact):
        self.artifact = artifact
        self.sources = []

    def __call__(self, source, options):
        self.sources.append(dict(source))
        return self.artifact


if __name__ == "__main__":
    unittest.main()
