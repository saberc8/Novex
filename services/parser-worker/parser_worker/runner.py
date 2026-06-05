from __future__ import annotations

from copy import deepcopy
from os import environ
from typing import Any, Mapping
from urllib.parse import urljoin, urlparse
from urllib.request import Request, urlopen
import json
import sys

from parser_worker.parse import is_native_structured_source, parse_request


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
        callback = post_parsed_document(
            dataset_id=positive_int(prepared_request.get("datasetId"), "datasetId"),
            payload=payload,
            backend_base_url=backend_base_url,
            backend_token=backend_token,
            http_post=http_post,
        )
        callback_status = "posted"
    elif status == "submitted":
        callback_status = "deferred"
    elif status == "failed":
        callback_status = "failed_no_callback"

    return {
        "status": status,
        "callbackStatus": callback_status,
        "callback": callback,
        "preparedRequest": prepared_request,
        "parserResult": parser_result,
    }


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


def default_http_post(url: str, *, headers: Mapping[str, str], json: Mapping[str, Any]) -> Mapping[str, Any]:
    body = json_dumps(json).encode("utf-8")
    request = Request(url, data=body, headers=dict(headers), method="POST")
    with urlopen(request, timeout=120) as response:
        response_body = response.read().decode("utf-8")
        return {
            "statusCode": getattr(response, "status", response.getcode()),
            "body": response_body,
        }


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
