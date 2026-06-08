from __future__ import annotations

from copy import deepcopy
from io import BytesIO
from os import environ
from typing import Any, Mapping
from urllib.parse import urljoin, urlparse
from urllib.request import Request, urlopen
from zipfile import ZipFile
import json
import sys

from parser_worker.parse import is_native_structured_source, parse_mineru_markdown_result, parse_request


def execute_parse_job(
    request: Mapping[str, Any],
    *,
    backend_base_url: str,
    backend_token: str = "",
    source_fetcher=None,
    http_post=None,
    pdf_converter=None,
    mineru_client=None,
    table_extractor=None,
) -> dict[str, Any]:
    prepared_request = prepare_parse_request(
        request,
        backend_base_url=backend_base_url,
        backend_token=backend_token,
        source_fetcher=source_fetcher,
    )
    parser_result = parse_request(
        prepared_request,
        pdf_converter=pdf_converter,
        mineru_client=mineru_client,
        table_extractor=table_extractor,
    )
    status = str(parser_result.get("status") or "")
    callback = None
    callback_status = "not_applicable"

    if status == "succeeded":
        payload = parsed_document_payload(prepared_request, parser_result)
        callback = ensure_backend_callback_ok(post_parsed_document(
            dataset_id=positive_int(prepared_request.get("datasetId"), "datasetId"),
            payload=payload,
            backend_base_url=backend_base_url,
            backend_token=backend_token,
            http_post=http_post,
        ))
        callback_status = "posted"
    elif status == "submitted":
        payload = parse_job_status_payload(prepared_request, parser_result, callback_status="deferred")
        callback = ensure_backend_callback_ok(post_parse_job_status(
            dataset_id=positive_int(prepared_request.get("datasetId"), "datasetId"),
            parser_job_id=positive_int(prepared_request.get("parserJobId"), "parserJobId"),
            payload=payload,
            backend_base_url=backend_base_url,
            backend_token=backend_token,
            http_post=http_post,
        ))
        callback_status = "deferred"
    elif status == "failed":
        payload = parse_job_status_payload(prepared_request, parser_result, callback_status="failed")
        callback = ensure_backend_callback_ok(post_parse_job_status(
            dataset_id=positive_int(prepared_request.get("datasetId"), "datasetId"),
            parser_job_id=positive_int(prepared_request.get("parserJobId"), "parserJobId"),
            payload=payload,
            backend_base_url=backend_base_url,
            backend_token=backend_token,
            http_post=http_post,
        ))
        callback_status = "failed"

    return {
        "status": status,
        "callbackStatus": callback_status,
        "callback": callback,
        "preparedRequest": prepared_request,
        "parserResult": parser_result,
    }


def complete_mineru_parse_job(
    request: Mapping[str, Any],
    *,
    task_id: str,
    backend_base_url: str,
    backend_token: str = "",
    mineru_client,
    zip_fetcher=None,
    http_post=None,
) -> dict[str, Any]:
    task = mineru_client.get_extract_task(task_id)
    state = str(task.state or "").strip().lower()
    if state != "done":
        status = "failed" if state in ("failed", "error") else "submitted"
        callback_status = "failed" if state in ("failed", "error") else "deferred"
        task_payload = mineru_task_payload(task)
        parser_result = {
            "tenantId": request.get("tenantId"),
            "datasetId": request.get("datasetId"),
            "documentId": request.get("documentId"),
            "parserJobId": request.get("parserJobId"),
            "status": status,
            "mineruTask": task_payload,
            "metadata": {"parser": "mineru"},
        }
        if status == "failed":
            parser_result["error"] = {
                "message": task.err_msg or "MinerU task failed",
                "mineruTask": task_payload,
            }
        callback = ensure_backend_callback_ok(post_parse_job_status(
            dataset_id=positive_int(request.get("datasetId"), "datasetId"),
            parser_job_id=positive_int(request.get("parserJobId"), "parserJobId"),
            payload=parse_job_status_payload(
                request,
                parser_result,
                callback_status=callback_status,
                mineru_task=task_payload,
            ),
            backend_base_url=backend_base_url,
            backend_token=backend_token,
            http_post=http_post,
        ))
        return {
            "status": status,
            "callbackStatus": callback_status,
            "mineruTask": task_payload,
            "callback": callback,
            "parserResult": parser_result,
        }
    if not str(task.full_zip_url or "").strip():
        raise ValueError("completed MinerU task is missing full_zip_url")

    zip_fetcher = zip_fetcher or default_zip_fetcher
    markdown = extract_mineru_markdown_from_zip(zip_fetcher(task.full_zip_url))
    parser_result = parse_mineru_markdown_result(
        request,
        markdown,
        mineru_metadata={
            "taskId": task.task_id,
            "state": task.state,
            "fullZipUrl": task.full_zip_url,
        },
    )
    payload = parsed_document_payload(request, parser_result)
    callback = ensure_backend_callback_ok(post_parsed_document(
        dataset_id=positive_int(request.get("datasetId"), "datasetId"),
        payload=payload,
        backend_base_url=backend_base_url,
        backend_token=backend_token,
        http_post=http_post,
    ))
    return {
        "status": parser_result.get("status"),
        "callbackStatus": "posted",
        "mineruTask": mineru_task_payload(task),
        "callback": callback,
        "parserResult": parser_result,
    }


def extract_mineru_markdown_from_zip(zip_bytes: bytes) -> str:
    candidates: list[tuple[int, str, str]] = []
    with ZipFile(BytesIO(zip_bytes)) as archive:
        for name in archive.namelist():
            lowered = name.lower()
            if not lowered.endswith((".md", ".markdown")):
                continue
            content = archive.read(name).decode("utf-8").strip()
            if not content:
                continue
            candidates.append((markdown_priority(lowered), name, content))
    if not candidates:
        raise ValueError("MinerU result zip does not contain markdown")
    candidates.sort(key=lambda item: (item[0], item[1]))
    return candidates[0][2]


def markdown_priority(name: str) -> int:
    basename = name.rsplit("/", 1)[-1]
    if basename in ("auto_full.md", "full.md", "result.md"):
        return 0
    if basename.endswith(".md"):
        return 1
    return 2


def prepare_parse_request(
    request: Mapping[str, Any],
    *,
    backend_base_url: str,
    backend_token: str = "",
    source_fetcher=None,
) -> dict[str, Any]:
    prepared = deepcopy(dict(request))
    source = dict(mapping_value(prepared.get("source"), "source"))
    uri = str(source.get("uri") or "").strip()
    if uri:
        source["uri"] = absolute_backend_url(uri, backend_base_url)

    if should_hydrate_text_source(source):
        source_fetcher = source_fetcher or default_source_fetcher(backend_token)
        content = source_fetcher(source["uri"], source)
        source["kind"] = "inlineText"
        source["content"] = str(content or "").strip().replace("\r\n", "\n")

    prepared["source"] = source
    return prepared


def should_hydrate_text_source(source: Mapping[str, Any]) -> bool:
    kind = str(source.get("kind") or "").strip()
    return kind in ("objectStorage", "remoteUrl") and is_native_structured_source(source)


def parsed_document_payload(request: Mapping[str, Any], parser_result: Mapping[str, Any]) -> dict[str, Any]:
    source = mapping_value(request.get("source"), "source")
    return {
        "name": non_empty(source.get("name"), f"document-{request.get('documentId')}"),
        "contentType": non_empty(source.get("contentType"), "text/plain"),
        "parserResult": parser_result,
    }


def parse_job_status_payload(
    request: Mapping[str, Any],
    parser_result: Mapping[str, Any],
    *,
    callback_status: str,
    mineru_task: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    status = non_empty(parser_result.get("status"), "submitted").lower()
    payload: dict[str, Any] = {
        "status": status,
        "callbackStatus": callback_status,
        "parserResult": dict(parser_result),
    }
    if mineru_task is not None:
        payload["mineruTask"] = dict(mineru_task)
    elif isinstance(parser_result.get("mineruTask"), Mapping):
        payload["mineruTask"] = dict(parser_result["mineruTask"])
    if parser_result.get("error"):
        payload["error"] = parser_result["error"]
    for source_key, result_key in [
        ("tenantId", "tenantId"),
        ("datasetId", "datasetId"),
        ("documentId", "documentId"),
        ("parserJobId", "parserJobId"),
    ]:
        payload["parserResult"].setdefault(result_key, request.get(source_key))
    return payload


def post_parsed_document(
    *,
    dataset_id: int,
    payload: Mapping[str, Any],
    backend_base_url: str,
    backend_token: str = "",
    http_post=None,
) -> Mapping[str, Any]:
    if not backend_base_url.strip():
        raise ValueError("backend_base_url is required for parser callback")
    url = absolute_backend_url(f"/ai/knowledge/datasets/{dataset_id}/documents/parsed", backend_base_url)
    headers = {"Content-Type": "application/json"}
    if backend_token.strip():
        headers["Authorization"] = f"Bearer {backend_token.strip()}"
    http_post = http_post or default_http_post
    return http_post(url, headers=headers, json=payload)


def post_parse_job_status(
    *,
    dataset_id: int,
    parser_job_id: int,
    payload: Mapping[str, Any],
    backend_base_url: str,
    backend_token: str = "",
    http_post=None,
) -> Mapping[str, Any]:
    if not backend_base_url.strip():
        raise ValueError("backend_base_url is required for parser status callback")
    url = absolute_backend_url(
        f"/ai/knowledge/datasets/{dataset_id}/parse-jobs/{parser_job_id}/status",
        backend_base_url,
    )
    headers = {"Content-Type": "application/json"}
    if backend_token.strip():
        headers["Authorization"] = f"Bearer {backend_token.strip()}"
    http_post = http_post or default_http_post
    return http_post(url, headers=headers, json=payload)


def default_http_post(url: str, *, headers: Mapping[str, str], json: Mapping[str, Any]) -> Mapping[str, Any]:
    body = json_dumps(json).encode("utf-8")
    request = Request(url, data=body, headers=dict(headers), method="POST")
    with urlopen(request, timeout=120) as response:
        response_body = response.read().decode("utf-8")
        return {
            "statusCode": getattr(response, "status", response.getcode()),
            "body": response_body,
        }


def ensure_backend_callback_ok(response: Mapping[str, Any]) -> Mapping[str, Any]:
    status_code = int(response.get("statusCode") or response.get("status") or 0)
    if status_code < 200 or status_code >= 300:
        raise RuntimeError(f"backend callback failed with HTTP {status_code}")

    body = response.get("body")
    if isinstance(body, str):
        try:
            body = json.loads(body)
        except json.JSONDecodeError:
            body = None
    if isinstance(body, Mapping):
        success = body.get("success")
        code = str(body.get("code") or "")
        if success is False or (code and code != "200"):
            message = str(body.get("msg") or body.get("message") or code or "unknown error")
            raise RuntimeError(f"backend callback failed: {message}")
    return response


def default_source_fetcher(backend_token: str = ""):
    def fetch(uri: str, _source: Mapping[str, Any]) -> str:
        headers = {}
        if backend_token.strip():
            headers["Authorization"] = f"Bearer {backend_token.strip()}"
        request = Request(uri, headers=headers)
        with urlopen(request, timeout=120) as response:
            raw = response.read()
            charset = response.headers.get_content_charset() or "utf-8"
            return raw.decode(charset)

    return fetch


def default_zip_fetcher(url: str) -> bytes:
    with urlopen(url, timeout=120) as response:
        return response.read()


def mineru_task_payload(task) -> dict[str, Any]:
    return {
        "taskId": task.task_id,
        "state": task.state,
        "fullZipUrl": task.full_zip_url,
        "errMsg": task.err_msg,
    }


def absolute_backend_url(value: str, backend_base_url: str) -> str:
    value = str(value or "").strip()
    if not value:
        return value
    if urlparse(value).scheme:
        return value
    if not backend_base_url.strip():
        return value
    return urljoin(backend_base_url.rstrip("/") + "/", value.lstrip("/"))


def mapping_value(value: Any, name: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise ValueError(f"{name} must be an object")
    return value


def non_empty(value: Any, default: str) -> str:
    normalized = str(value or "").strip()
    return normalized or default


def positive_int(value: Any, name: str) -> int:
    try:
        parsed = int(value)
    except (TypeError, ValueError) as error:
        raise ValueError(f"{name} must be a positive integer") from error
    if parsed <= 0:
        raise ValueError(f"{name} must be a positive integer")
    return parsed


def json_dumps(value: Mapping[str, Any]) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True)


def main() -> None:
    request = json.load(sys.stdin)
    result = execute_parse_job(
        request,
        backend_base_url=environ.get("PARSER_BACKEND_BASE_URL", ""),
        backend_token=environ.get("PARSER_BACKEND_TOKEN", ""),
    )
    print(json_dumps(result))


if __name__ == "__main__":
    main()
