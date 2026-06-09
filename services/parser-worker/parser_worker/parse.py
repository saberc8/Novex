from __future__ import annotations

from dataclasses import dataclass, field
from hashlib import sha256
from pathlib import Path
from typing import Any, Mapping
import json
import re
import sys

from parser_worker.config import load_config
from parser_worker.mineru_client import MineruClient


DEFAULT_MAX_CHUNK_CHARS = 1200
DEFAULT_CHUNK_OVERLAP_CHARS = 120
PARSER_NAME = "novex-parser-worker-local"
MINERU_PARSER_NAME = "mineru"


@dataclass
class ParsedBlock:
    block_id: str
    block_type: str
    text: str
    page_no: int | None = None
    section_path: list[str] = field(default_factory=list)
    bbox: dict[str, int] | None = None
    table_header: list[str] = field(default_factory=list)
    image_access_keys: list[str] = field(default_factory=list)

    def to_contract(self) -> dict[str, Any]:
        return {
            "blockId": self.block_id,
            "type": self.block_type,
            "text": self.text,
            "pageNo": self.page_no,
            "sectionPath": self.section_path,
            "bbox": self.bbox,
        }


def parse_request(
    request: Mapping[str, Any],
    *,
    pdf_converter=None,
    mineru_client=None,
    table_extractor=None,
) -> dict[str, Any]:
    source = mapping_value(request.get("source"), "source")
    if is_tabular_binary_source(source):
        result = parse_table_extractor_request(request, table_extractor=table_extractor)
        if isinstance(result.get("metadata"), dict):
            result["metadata"]["strategy"] = "native_structured"
            result["metadata"]["pdfFirst"] = False
        return result
    if is_native_structured_source(source):
        result = parse_local_request(request)
        if isinstance(result.get("metadata"), dict):
            result["metadata"]["strategy"] = "native_structured"
            result["metadata"]["pdfFirst"] = False
        return result
    return submit_mineru_pdf_parse(
        request,
        pdf_converter=pdf_converter,
        mineru_client=mineru_client,
    )


def parse_table_extractor_request(
    request: Mapping[str, Any],
    *,
    table_extractor=None,
) -> dict[str, Any]:
    tenant_id = positive_int(request.get("tenantId"), "tenantId")
    dataset_id = positive_int(request.get("datasetId"), "datasetId")
    document_id = positive_int(request.get("documentId"), "documentId")
    parser_job_id = positive_int(request.get("parserJobId"), "parserJobId")
    source = mapping_value(request.get("source"), "source")
    options = request.get("options") if isinstance(request.get("options"), Mapping) else {}
    if table_extractor is None:
        return {
            "tenantId": tenant_id,
            "datasetId": dataset_id,
            "documentId": document_id,
            "parserJobId": parser_job_id,
            "status": "failed",
            "error": {
                "code": "table_extractor_required",
                "message": "table extractor is required for XLS/XLSX/ODS sources",
                "retryable": False,
            },
            "blocks": [],
            "chunks": [],
            "metadata": {
                "parser": PARSER_NAME,
                "strategy": "native_structured",
                "pdfFirst": False,
                "sourceHash": non_empty_str(source.get("sourceHash"), None),
                "warnings": ["table extractor is required for XLS/XLSX/ODS sources"],
            },
        }

    try:
        artifact = mapping_value(table_extractor(source, options), "table extraction artifact")
        local_request = {
            "tenantId": tenant_id,
            "datasetId": dataset_id,
            "documentId": document_id,
            "parserJobId": parser_job_id,
            "source": {
                "kind": "inlineText",
                "contentType": non_empty_str(artifact.get("contentType"), "text/csv"),
                "name": non_empty_str(artifact.get("name"), csv_name(non_empty_str(source.get("name"), "table"))),
                "content": str(artifact.get("content") or ""),
                "sourceHash": non_empty_str(artifact.get("sourceHash"), non_empty_str(source.get("sourceHash"), None)),
            },
            "options": options,
        }
        result = parse_local_request(local_request)
        if isinstance(result.get("metadata"), dict):
            result["metadata"]["sourceName"] = non_empty_str(source.get("name"), "")
            result["metadata"]["sourceContentType"] = non_empty_str(source.get("contentType"), "")
        return result
    except Exception as error:
        return {
            "tenantId": tenant_id,
            "datasetId": dataset_id,
            "documentId": document_id,
            "parserJobId": parser_job_id,
            "status": "failed",
            "error": {
                "code": "table_extract_failed",
                "message": str(error),
                "retryable": False,
            },
            "blocks": [],
            "chunks": [],
            "metadata": {
                "parser": PARSER_NAME,
                "strategy": "native_structured",
                "pdfFirst": False,
                "sourceHash": non_empty_str(source.get("sourceHash"), None),
                "warnings": [str(error)],
            },
        }


def parse_local_request(request: Mapping[str, Any]) -> dict[str, Any]:
    tenant_id = positive_int(request.get("tenantId"), "tenantId")
    dataset_id = positive_int(request.get("datasetId"), "datasetId")
    document_id = positive_int(request.get("documentId"), "documentId")
    parser_job_id = positive_int(request.get("parserJobId"), "parserJobId")
    source = mapping_value(request.get("source"), "source")
    options = request.get("options") if isinstance(request.get("options"), Mapping) else {}

    try:
        content = load_source_text(source)
        content_type = non_empty_str(source.get("contentType"), "text/plain").lower()
        source_name = non_empty_str(source.get("name"), "")
        max_chunk_chars = positive_int(
            options.get("maxChunkChars", DEFAULT_MAX_CHUNK_CHARS),
            "options.maxChunkChars",
        )
        chunk_overlap_chars = max(
            0,
            min(
                int(options.get("chunkOverlapChars", DEFAULT_CHUNK_OVERLAP_CHARS) or 0),
                max_chunk_chars - 1,
            ),
        )
        blocks = parse_blocks(content, source_name, content_type)
        chunks = build_chunks(
            document_id=document_id,
            source_name=source_name,
            content_type=content_type,
            blocks=blocks,
            max_chunk_chars=max_chunk_chars,
            chunk_overlap_chars=chunk_overlap_chars,
        )
        if not chunks:
            raise ValueError("parser result has no chunks")

        return {
            "tenantId": tenant_id,
            "datasetId": dataset_id,
            "documentId": document_id,
            "parserJobId": parser_job_id,
            "status": "succeeded",
            "error": None,
            "blocks": [block.to_contract() for block in blocks],
            "chunks": chunks,
            "metadata": {
                "parser": PARSER_NAME,
                "pageCount": page_count(blocks),
                "lineCount": line_count(content),
                "sourceHash": non_empty_str(source.get("sourceHash"), sha256_hex(content)),
                "warnings": [],
            },
        }
    except Exception as error:
        return {
            "tenantId": tenant_id,
            "datasetId": dataset_id,
            "documentId": document_id,
            "parserJobId": parser_job_id,
            "status": "failed",
            "error": {
                "code": "local_parse_failed",
                "message": str(error),
                "retryable": False,
            },
            "blocks": [],
            "chunks": [],
            "metadata": {
                "parser": PARSER_NAME,
                "pageCount": None,
                "lineCount": None,
                "sourceHash": non_empty_str(source.get("sourceHash"), None),
                "warnings": [str(error)],
            },
        }


def submit_mineru_pdf_parse(
    request: Mapping[str, Any],
    *,
    pdf_converter=None,
    mineru_client=None,
) -> dict[str, Any]:
    tenant_id = positive_int(request.get("tenantId"), "tenantId")
    dataset_id = positive_int(request.get("datasetId"), "datasetId")
    document_id = positive_int(request.get("documentId"), "documentId")
    parser_job_id = positive_int(request.get("parserJobId"), "parserJobId")
    source = mapping_value(request.get("source"), "source")
    options = request.get("options") if isinstance(request.get("options"), Mapping) else {}

    try:
        normalized = normalize_pdf_source(source, options, pdf_converter)
        mineru_client = mineru_client or default_mineru_client()
        task = mineru_client.create_extract_task(
            normalized["uri"],
            file_name=normalized["name"],
            data_id=str(parser_job_id),
            model_version=non_empty_str(options.get("mineruModelVersion"), "vlm"),
            is_ocr=bool(options.get("ocr", is_image_source(source))),
            enable_formula=bool(options.get("extractFormula", True)),
            enable_table=bool(options.get("extractTables", True)),
            language=non_empty_str(options.get("language"), "ch"),
        )
        return {
            "tenantId": tenant_id,
            "datasetId": dataset_id,
            "documentId": document_id,
            "parserJobId": parser_job_id,
            "status": "submitted",
            "error": None,
            "normalizedSource": normalized,
            "mineruTask": {
                "taskId": task.task_id,
                "state": task.state,
                "fullZipUrl": task.full_zip_url,
            },
            "blocks": [],
            "chunks": [],
            "metadata": {
                "parser": MINERU_PARSER_NAME,
                "strategy": "mineru_pdf",
                "pdfFirst": True,
                "sourceName": non_empty_str(source.get("name"), ""),
                "sourceContentType": non_empty_str(source.get("contentType"), ""),
                "sourceHash": normalized.get("sourceHash") or non_empty_str(source.get("sourceHash"), None),
                "warnings": [],
            },
        }
    except Exception as error:
        return {
            "tenantId": tenant_id,
            "datasetId": dataset_id,
            "documentId": document_id,
            "parserJobId": parser_job_id,
            "status": "failed",
            "error": {
                "code": "mineru_submit_failed",
                "message": str(error),
                "retryable": False,
            },
            "normalizedSource": None,
            "mineruTask": None,
            "blocks": [],
            "chunks": [],
            "metadata": {
                "parser": MINERU_PARSER_NAME,
                "strategy": "mineru_pdf",
                "pdfFirst": True,
                "sourceName": non_empty_str(source.get("name"), ""),
                "sourceContentType": non_empty_str(source.get("contentType"), ""),
                "sourceHash": non_empty_str(source.get("sourceHash"), None),
                "warnings": [str(error)],
            },
        }


def parse_mineru_markdown_result(
    request: Mapping[str, Any],
    markdown: str,
    *,
    mineru_metadata: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    source = mapping_value(request.get("source"), "source")
    mineru_metadata = mineru_metadata or {}
    local_request = {
        "tenantId": request.get("tenantId"),
        "datasetId": request.get("datasetId"),
        "documentId": request.get("documentId"),
        "parserJobId": request.get("parserJobId"),
        "source": {
            "kind": "inlineText",
            "contentType": "text/markdown",
            "name": non_empty_str(source.get("name"), "mineru-result.md"),
            "content": markdown,
            "sourceHash": non_empty_str(mineru_metadata.get("sourceHash"), non_empty_str(source.get("sourceHash"), None)),
        },
        "options": request.get("options") if isinstance(request.get("options"), Mapping) else {},
    }
    result = parse_local_request(local_request)
    if isinstance(result.get("metadata"), dict):
        result["metadata"]["parser"] = MINERU_PARSER_NAME
        result["metadata"]["strategy"] = "mineru_layout"
        result["metadata"]["pdfFirst"] = True
        if "pageCount" in mineru_metadata:
            result["metadata"]["pageCount"] = mineru_metadata.get("pageCount")
        if "sourceHash" in mineru_metadata:
            result["metadata"]["sourceHash"] = mineru_metadata.get("sourceHash")
        result["metadata"]["mineru"] = {
            key: value
            for key, value in mineru_metadata.items()
            if key not in ("pageCount", "sourceHash")
        }
    return result


def normalize_pdf_source(source: Mapping[str, Any], options: Mapping[str, Any], pdf_converter) -> dict[str, Any]:
    if is_pdf_source(source):
        uri = non_empty_str(source.get("uri"), "")
        if not uri:
            raise ValueError("PDF MinerU source requires uri")
        return {
            "kind": non_empty_str(source.get("kind"), ""),
            "contentType": "application/pdf",
            "name": non_empty_str(source.get("name"), "document.pdf"),
            "uri": uri,
            "sourceHash": non_empty_str(source.get("sourceHash"), None),
            "converted": False,
        }

    if is_image_source(source):
        uri = non_empty_str(source.get("uri"), "")
        if not uri:
            raise ValueError("image MinerU source requires uri")
        return {
            "kind": non_empty_str(source.get("kind"), ""),
            "contentType": "application/pdf",
            "name": pdf_name(non_empty_str(source.get("name"), "image")),
            "uri": uri,
            "sourceHash": non_empty_str(source.get("sourceHash"), None),
            "converted": False,
        }

    if pdf_converter is None:
        raise ValueError("PDF converter is required for Office/layout-heavy sources")
    artifact = mapping_value(pdf_converter(source, options), "converted PDF artifact")
    uri = non_empty_str(artifact.get("uri"), "")
    if not uri:
        raise ValueError("converted PDF artifact requires uri")
    return {
        "kind": "objectStorage",
        "contentType": "application/pdf",
        "name": non_empty_str(artifact.get("name"), pdf_name(non_empty_str(source.get("name"), "document"))),
        "uri": uri,
        "sourceHash": non_empty_str(artifact.get("sourceHash"), non_empty_str(source.get("sourceHash"), None)),
        "converted": True,
        "original": {
            "kind": non_empty_str(source.get("kind"), ""),
            "contentType": non_empty_str(source.get("contentType"), ""),
            "name": non_empty_str(source.get("name"), ""),
            "uri": non_empty_str(source.get("uri"), ""),
        },
    }


def default_mineru_client() -> MineruClient:
    config = load_config()
    if not config.mineru.configured:
        raise ValueError("MINERU_TOKEN is required for MinerU parsing")
    return MineruClient(
        token=config.mineru.token,
        timeout_seconds=config.mineru.timeout_seconds,
    )


def load_source_text(source: Mapping[str, Any]) -> str:
    kind = non_empty_str(source.get("kind"), "")
    if kind == "inlineText":
        return str(source.get("content") or "").strip().replace("\r\n", "\n")
    if kind == "localFile":
        uri = non_empty_str(source.get("uri"), "")
        if not uri:
            raise ValueError("localFile source requires uri")
        return Path(uri).read_text(encoding="utf-8").strip().replace("\r\n", "\n")
    raise ValueError(f"unsupported source kind: {kind}")


def is_native_structured_source(source: Mapping[str, Any]) -> bool:
    content_type = non_empty_str(source.get("contentType"), "").lower()
    name = non_empty_str(source.get("name"), "").lower()
    kind = non_empty_str(source.get("kind"), "")
    if kind == "inlineText":
        return not is_pdf_source(source) and not is_office_source(source) and not is_image_source(source)
    return is_text_source_name(name) or is_native_text_content_type(content_type)


def is_native_text_content_type(content_type: str) -> bool:
    return any(
        marker in content_type
        for marker in (
            "text/plain",
            "text/markdown",
            "text/html",
            "application/json",
            "text/csv",
            "tab-separated-values",
            "application/x-ndjson",
            "application/xml",
            "text/xml",
        )
    )


def is_text_source_name(name: str) -> bool:
    return name.endswith(
        (
            ".txt",
            ".md",
            ".markdown",
            ".html",
            ".htm",
            ".csv",
            ".tsv",
            ".json",
            ".jsonl",
            ".ndjson",
            ".xml",
            ".log",
            ".rs",
            ".py",
            ".ts",
            ".tsx",
            ".js",
            ".jsx",
            ".java",
            ".go",
            ".sql",
            ".yaml",
            ".yml",
            ".toml",
        )
    )


def is_pdf_source(source: Mapping[str, Any]) -> bool:
    content_type = non_empty_str(source.get("contentType"), "").lower()
    name = non_empty_str(source.get("name"), "").lower()
    return content_type == "application/pdf" or name.endswith(".pdf")


def is_office_source(source: Mapping[str, Any]) -> bool:
    content_type = non_empty_str(source.get("contentType"), "").lower()
    name = non_empty_str(source.get("name"), "").lower()
    return (
        any(marker in content_type for marker in ("wordprocessingml", "presentationml", "msword", "powerpoint", "opendocument.text", "opendocument.presentation"))
        or name.endswith((".doc", ".docx", ".ppt", ".pptx", ".odt", ".odp"))
    )


def is_tabular_binary_source(source: Mapping[str, Any]) -> bool:
    content_type = non_empty_str(source.get("contentType"), "").lower()
    name = non_empty_str(source.get("name"), "").lower()
    return (
        any(marker in content_type for marker in ("spreadsheetml", "excel", "opendocument.spreadsheet"))
        or name.endswith((".xls", ".xlsx", ".ods"))
    )


def is_image_source(source: Mapping[str, Any]) -> bool:
    content_type = non_empty_str(source.get("contentType"), "").lower()
    name = non_empty_str(source.get("name"), "").lower()
    return content_type.startswith("image/") or name.endswith((".png", ".jpg", ".jpeg", ".webp", ".tif", ".tiff", ".bmp"))


def pdf_name(name: str) -> str:
    name = name.strip() or "document"
    stem = name.rsplit("/", 1)[-1]
    if stem.lower().endswith(".pdf"):
        return stem
    if "." in stem:
        stem = stem.rsplit(".", 1)[0]
    return f"{stem}.pdf"


def csv_name(name: str) -> str:
    name = name.strip() or "table"
    stem = name.rsplit("/", 1)[-1]
    if stem.lower().endswith((".csv", ".tsv")):
        return stem
    if "." in stem:
        stem = stem.rsplit(".", 1)[0]
    return f"{stem}.csv"


def parse_blocks(content: str, source_name: str, content_type: str) -> list[ParsedBlock]:
    if is_table_source(source_name, content_type):
        return parse_table_blocks(content)
    return parse_text_blocks(content)


def parse_table_blocks(content: str) -> list[ParsedBlock]:
    lines = clean_lines(content)
    if not lines:
        return []
    header = split_table_cells(lines[0])
    return [
        ParsedBlock(
            block_id="b-0",
            block_type="table",
            text="\n".join(lines),
            table_header=header,
        )
    ]


def parse_text_blocks(content: str) -> list[ParsedBlock]:
    blocks: list[ParsedBlock] = []
    paragraph: list[str] = []
    section_path: list[str] = []
    page_no: int | None = None

    def next_block_id() -> str:
        return f"b-{len(blocks)}"

    def flush_paragraph() -> None:
        if not paragraph:
            return
        text = "\n".join(paragraph).strip()
        paragraph.clear()
        if text:
            blocks.append(
                ParsedBlock(
                    block_id=next_block_id(),
                    block_type="paragraph",
                    text=text,
                    page_no=page_no,
                    section_path=section_path.copy(),
                )
            )

    lines = content.splitlines()
    index = 0
    while index < len(lines):
        line = lines[index]
        trimmed = line.strip()
        if not trimmed:
            flush_paragraph()
            index += 1
            continue

        heading = markdown_heading(trimmed)
        if heading:
            flush_paragraph()
            level, title = heading
            section_path = section_path[: max(level - 1, 0)]
            section_path.append(title)
            blocks.append(
                ParsedBlock(
                    block_id=next_block_id(),
                    block_type="title",
                    text=title,
                    page_no=page_no,
                    section_path=section_path.copy(),
                )
            )
            index += 1
            continue

        page = page_marker(trimmed)
        if page is not None:
            flush_paragraph()
            page_no = page
            index += 1
            continue

        image = image_marker(trimmed, next_block_id(), page_no, section_path)
        if image:
            flush_paragraph()
            blocks.append(image)
            index += 1
            continue

        if is_markdown_table_line(trimmed):
            flush_paragraph()
            table_lines = [trimmed]
            index += 1
            while index < len(lines) and is_markdown_table_line(lines[index].strip()):
                table_lines.append(lines[index].strip())
                index += 1
            table_text, table_header = normalize_markdown_table(table_lines)
            if table_text:
                blocks.append(
                    ParsedBlock(
                        block_id=next_block_id(),
                        block_type="table",
                        text=table_text,
                        page_no=page_no,
                        section_path=section_path.copy(),
                        table_header=table_header,
                    )
                )
            continue

        paragraph.append(trimmed)
        index += 1

    flush_paragraph()
    if not blocks and content.strip():
        blocks.append(ParsedBlock(block_id="b-0", block_type="paragraph", text=content.strip()))
    return blocks


def is_markdown_table_line(line: str) -> bool:
    return line.startswith("|") and line.endswith("|") and line.count("|") >= 2


def normalize_markdown_table(lines: list[str]) -> tuple[str, list[str]]:
    rows = []
    for line in lines:
        cells = markdown_table_cells(line)
        if not cells or is_markdown_table_separator(cells):
            continue
        rows.append(cells)
    if not rows:
        return "", []
    header = rows[0]
    normalized_lines = [",".join(row) for row in rows]
    return "\n".join(normalized_lines), header


def markdown_table_cells(line: str) -> list[str]:
    return [cell.strip() for cell in line.strip().strip("|").split("|")]


def is_markdown_table_separator(cells: list[str]) -> bool:
    return all(re.fullmatch(r":?-{3,}:?", cell.strip()) for cell in cells)


def build_chunks(
    *,
    document_id: int,
    source_name: str,
    content_type: str,
    blocks: list[ParsedBlock],
    max_chunk_chars: int,
    chunk_overlap_chars: int,
) -> list[dict[str, Any]]:
    chunks: list[dict[str, Any]] = []
    for segment_index, block in enumerate(blocks):
        if block.block_type in ("title", "pageBreak") or not block.text.strip():
            continue
        segment_type = segment_type_for_block(block)
        parts = split_table_block(block.text, max_chunk_chars) if segment_type == "table" else split_text_block(
            block.text,
            max_chunk_chars,
            chunk_overlap_chars,
        )
        for part in parts:
            if not part.strip():
                continue
            chunk_index = len(chunks)
            chunk_uid = f"{document_id}:{chunk_index}"
            table_header = block.table_header if segment_type == "table" else []
            semantic_search_text = build_semantic_search_text(
                raw_text=part,
                source_name=source_name,
                section_path=block.section_path,
                table_header=table_header,
            )
            chunks.append(
                {
                    "chunkUid": chunk_uid,
                    "chunkIndex": chunk_index,
                    "text": part,
                    "semanticSearchText": semantic_search_text,
                    "segmentType": segment_type,
                    "tableHeader": table_header,
                    "imageAccessKeys": unique_non_empty(block.image_access_keys),
                    "contentRole": infer_content_role(block.section_path, part),
                    "displayCapability": display_capability(segment_type, block),
                    "metadata": {
                        "sourceBlockType": block.block_type,
                        "sourceContentType": content_type,
                        "segmentIndex": segment_index,
                    },
                    "tokenCount": tokenish_count(semantic_search_text),
                    "citation": {
                        "documentId": str(document_id),
                        "chunkId": chunk_uid,
                        "pageNo": block.page_no,
                        "sectionPath": block.section_path,
                        "blockIds": [block.block_id],
                    },
                }
            )
    return chunks


def split_table_block(text: str, max_chunk_chars: int) -> list[str]:
    lines = clean_lines(text)
    if not lines:
        return []
    header = lines[0]
    if len(lines) == 1:
        return [header]

    chunks: list[str] = []
    current = header
    for row in lines[1:]:
        single_row = f"{header}\n{row}"
        if len(single_row) > max_chunk_chars:
            if current != header:
                chunks.append(current)
                current = header
            chunks.extend(f"{header}\n{part}" for part in split_by_chars(row, max(1, max_chunk_chars - len(header) - 1), 0))
            continue

        candidate = f"{current}\n{row}"
        if current != header and len(candidate) > max_chunk_chars:
            chunks.append(current)
            current = single_row
        elif current == header:
            current = single_row
        else:
            current = candidate
    if current != header:
        chunks.append(current)
    return chunks


def split_text_block(text: str, max_chunk_chars: int, overlap_chars: int) -> list[str]:
    text = text.strip()
    if not text:
        return []
    if len(text) <= max_chunk_chars:
        return [text]

    sentences = split_sentence_units(text)
    if len(sentences) <= 1:
        return split_by_chars(text, max_chunk_chars, overlap_chars)

    chunks: list[str] = []
    current = ""
    for sentence in sentences:
        if len(sentence) > max_chunk_chars:
            if current:
                chunks.append(current)
                current = ""
            chunks.extend(split_by_chars(sentence, max_chunk_chars, overlap_chars))
            continue
        candidate = sentence if not current else f"{current} {sentence}"
        if len(candidate) <= max_chunk_chars:
            current = candidate
        else:
            chunks.append(current)
            current = sentence
    if current:
        chunks.append(current)
    return chunks


def split_by_chars(text: str, max_chars: int, overlap_chars: int) -> list[str]:
    step = max(1, max_chars - min(overlap_chars, max_chars - 1))
    chunks = []
    start = 0
    while start < len(text):
        end = min(start + max_chars, len(text))
        part = text[start:end].strip()
        if part:
            chunks.append(part)
        if end == len(text):
            break
        start += step
    return chunks


def split_sentence_units(text: str) -> list[str]:
    units = []
    current = []
    for index, character in enumerate(text):
        current.append(character)
        next_char = text[index + 1] if index + 1 < len(text) else ""
        if character in "。！？!?" or (character == "." and (not next_char or next_char.isspace())):
            unit = "".join(current).strip()
            if unit:
                units.append(unit)
            current = []
    tail = "".join(current).strip()
    if tail:
        units.append(tail)
    return units


def build_semantic_search_text(
    *,
    raw_text: str,
    source_name: str,
    section_path: list[str],
    table_header: list[str],
) -> str:
    parts = []
    seen = set()
    append_search_part(parts, seen, source_name)
    append_search_part(parts, seen, " / ".join(section_path))
    append_search_part(parts, seen, " ".join(table_header))
    append_search_part(parts, seen, clean_search_text(raw_text))
    return "\n".join(parts)


def clean_search_text(text: str) -> str:
    lines = [line for line in text.splitlines() if not is_low_value_image_caption(line)]
    text = "\n".join(lines)
    text = re.sub(r"\\[A-Za-z]+", " ", text)
    text = re.sub(r"[{}\[\]]", " ", text)
    text = re.sub(
        r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b",
        " ",
        text,
    )
    return " ".join(text.split())


def append_search_part(parts: list[str], seen: set[str], value: str) -> None:
    normalized = " ".join(str(value or "").split()).strip()
    if not normalized:
        return
    key = normalized.lower()
    if key not in seen:
        seen.add(key)
        parts.append(normalized)


def image_marker(
    line: str,
    block_id: str,
    current_page_no: int | None,
    section_path: list[str],
) -> ParsedBlock | None:
    stripped = line.strip()
    if not stripped.lower().startswith("[[image:") or not stripped.endswith("]]"):
        return None
    payload = stripped[len("[[image:") : -2].strip()
    image_key = first_field(payload, ("key", "image_key", "access_key"))
    caption = tail_field(payload, "caption") or tail_field(payload, "alt") or image_key or "Image evidence"
    page_no = int_field(payload, ("page", "page_no")) or current_page_no
    bbox = bbox_field(payload, ("bbox", "coordinates"))
    return ParsedBlock(
        block_id=block_id,
        block_type="image",
        text=caption,
        page_no=page_no,
        section_path=section_path.copy(),
        bbox=bbox,
        image_access_keys=[image_key] if image_key else [],
    )


def first_field(payload: str, names: tuple[str, ...]) -> str:
    for name in names:
        match = re.search(rf"(?:^|\s){re.escape(name)}=([^\s]+)", payload, flags=re.IGNORECASE)
        if match:
            return match.group(1).strip().strip("\"'")
    return ""


def tail_field(payload: str, name: str) -> str:
    match = re.search(rf"(?:^|\s){re.escape(name)}=", payload, flags=re.IGNORECASE)
    if not match:
        return ""
    return payload[match.end() :].strip().strip("\"'")


def int_field(payload: str, names: tuple[str, ...]) -> int | None:
    value = first_field(payload, names)
    if not value:
        return None
    try:
        parsed = int(value)
    except ValueError:
        return None
    return parsed if parsed > 0 else None


def bbox_field(payload: str, names: tuple[str, ...]) -> dict[str, int] | None:
    value = first_field(payload, names)
    if not value:
        return None
    parts = [part.strip() for part in value.split(",")]
    if len(parts) != 4:
        return None
    try:
        x, y, width, height = (int(float(part)) for part in parts)
    except ValueError:
        return None
    return {"x": x, "y": y, "width": width, "height": height}


def markdown_heading(line: str) -> tuple[int, str] | None:
    match = re.match(r"^(#{1,6})\s+(.+?)\s*$", line)
    if not match:
        return None
    return len(match.group(1)), match.group(2).strip()


def page_marker(line: str) -> int | None:
    if len(line) > 64:
        return None
    match = re.match(r"^\[\[page:\s*(\d+)\]\]$", line, flags=re.IGNORECASE)
    if not match:
        match = re.search(r"(?:page|页)\D*(\d+)", line, flags=re.IGNORECASE)
    if not match:
        return None
    page = int(match.group(1))
    return page if page > 0 else None


def is_table_source(source_name: str, content_type: str) -> bool:
    lowered_type = content_type.lower()
    lowered_name = source_name.lower()
    return (
        "csv" in lowered_type
        or "spreadsheet" in lowered_type
        or "excel" in lowered_type
        or "tab-separated-values" in lowered_type
        or lowered_name.endswith((".csv", ".tsv", ".xls", ".xlsx"))
    )


def split_table_cells(line: str) -> list[str]:
    delimiter = "\t" if "\t" in line else "|" if "|" in line and "," not in line else ","
    return unique_non_empty(cell.strip().strip("|") for cell in line.split(delimiter))


def segment_type_for_block(block: ParsedBlock) -> str:
    if block.block_type == "table":
        return "table"
    if block.block_type == "image":
        return "image"
    return "text"


def display_capability(segment_type: str, block: ParsedBlock) -> str:
    if block.page_no is not None or block.bbox is not None:
        return "precise_anchor"
    if segment_type == "table":
        return "row_only"
    return "text_only"


def infer_content_role(section_path: list[str], text: str) -> str:
    haystack = f"{' '.join(section_path)} {text}".lower()
    if any(marker in haystack for marker in ("faq", "问答", "常见问题")):
        return "summary_faq"
    if any(marker in haystack for marker in ("test", "example", "测试", "示例")):
        return "test_case"
    return "canonical"


def clean_lines(content: str) -> list[str]:
    return [line.strip() for line in content.splitlines() if line.strip()]


def unique_non_empty(values) -> list[str]:
    result = []
    seen = set()
    for value in values:
        normalized = str(value or "").strip()
        if normalized and normalized not in seen:
            seen.add(normalized)
            result.append(normalized)
    return result


def is_low_value_image_caption(line: str) -> bool:
    lowered = line.strip().lower()
    return bool(lowered) and ("image" in lowered or "图片" in lowered) and (
        "fallback" in lowered
        or "placeholder" in lowered
        or "占位" in lowered
        or lowered in ("[image]", "<image>")
    )


def tokenish_count(text: str) -> int:
    tokens = re.findall(r"\w+", text, flags=re.UNICODE)
    if tokens:
        return len(tokens)
    return len(text.strip())


def line_count(content: str) -> int:
    return len([line for line in content.splitlines() if line.strip()])


def page_count(blocks: list[ParsedBlock]) -> int | None:
    pages = [block.page_no for block in blocks if block.page_no is not None]
    return max(pages) if pages else None


def sha256_hex(content: str) -> str:
    return sha256(content.encode("utf-8")).hexdigest()


def non_empty_str(value: Any, default: Any) -> Any:
    if value is None:
        return default
    normalized = str(value).strip()
    return normalized if normalized else default


def mapping_value(value: Any, name: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise ValueError(f"{name} must be an object")
    return value


def positive_int(value: Any, name: str) -> int:
    try:
        parsed = int(value)
    except (TypeError, ValueError) as error:
        raise ValueError(f"{name} must be a positive integer") from error
    if parsed <= 0:
        raise ValueError(f"{name} must be a positive integer")
    return parsed


def main() -> None:
    request = json.load(sys.stdin)
    print(json.dumps(parse_request(request), ensure_ascii=False, sort_keys=True))


if __name__ == "__main__":
    main()
