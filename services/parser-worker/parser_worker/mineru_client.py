from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Mapping
from urllib.error import HTTPError, URLError
from urllib.parse import unquote, urljoin, urlparse
from urllib.request import Request, urlopen
import ipaddress
import http.client
import json
import ssl


DEFAULT_MINERU_BASE_URL = "https://mineru.net"
DEFAULT_MINERU_LANGUAGE = "ch"
DEFAULT_MINERU_MODEL_VERSION = "vlm"


class MineruError(RuntimeError):
    pass


@dataclass(frozen=True)
class MineruRequest:
    method: str
    base_url: str
    path: str
    headers: Mapping[str, str]
    json: dict[str, Any] | None = None
    body: bytes | None = None
    timeout_seconds: int = 120


@dataclass(frozen=True)
class MineruTask:
    task_id: str
    state: str
    full_zip_url: str = ""
    err_msg: str = ""
    raw: Mapping[str, Any] | None = None


MineruTransport = Callable[[MineruRequest], Mapping[str, Any]]
SourceReader = Callable[[str], bytes]


class MineruClient:
    def __init__(
        self,
        token: str,
        base_url: str = DEFAULT_MINERU_BASE_URL,
        timeout_seconds: int = 120,
        transport: MineruTransport | None = None,
        source_reader: SourceReader | None = None,
    ) -> None:
        token = token.strip()
        if not token:
            raise MineruError("MINERU_TOKEN is required")
        self._token = token
        self._base_url = base_url.rstrip("/")
        self._timeout_seconds = timeout_seconds
        self._transport = transport or urllib_transport
        self._source_reader = source_reader or download_source_bytes

    def create_extract_task(
        self,
        source_url: str,
        *,
        file_name: str = "",
        data_id: str = "",
        model_version: str = DEFAULT_MINERU_MODEL_VERSION,
        is_ocr: bool = False,
        enable_formula: bool = True,
        enable_table: bool = True,
        language: str = DEFAULT_MINERU_LANGUAGE,
    ) -> MineruTask:
        source_url = source_url.strip()
        if not source_url:
            raise MineruError("source url is required")

        if should_upload_source_url(source_url):
            return self._create_extract_task_from_file_upload(
                source_url,
                file_name=file_name,
                data_id=data_id,
                model_version=model_version,
                is_ocr=is_ocr,
                enable_formula=enable_formula,
                enable_table=enable_table,
                language=language,
            )

        request = MineruRequest(
            method="POST",
            base_url=self._base_url,
            path="/api/v4/extract/task",
            headers=self._headers(),
            json={
                "url": source_url,
                "model_version": model_version,
                "is_ocr": is_ocr,
                "enable_formula": enable_formula,
                "enable_table": enable_table,
                "language": language,
            },
            timeout_seconds=self._timeout_seconds,
        )
        return task_from_response(self._transport(request), self._token)

    def get_extract_task(self, task_id: str) -> MineruTask:
        task_id = task_id.strip()
        if not task_id:
            raise MineruError("task id is required")
        if task_id.startswith("batch:"):
            return self._get_extract_batch_task(task_id.removeprefix("batch:"))

        request = MineruRequest(
            method="GET",
            base_url=self._base_url,
            path=f"/api/v4/extract/task/{task_id}",
            headers=self._headers(),
            timeout_seconds=self._timeout_seconds,
        )
        return task_from_response(self._transport(request), self._token)

    def _create_extract_task_from_file_upload(
        self,
        source_url: str,
        *,
        file_name: str,
        data_id: str,
        model_version: str,
        is_ocr: bool,
        enable_formula: bool,
        enable_table: bool,
        language: str,
    ) -> MineruTask:
        upload_name = non_empty(file_name, file_name_from_url(source_url))
        apply_request = MineruRequest(
            method="POST",
            base_url=self._base_url,
            path="/api/v4/file-urls/batch",
            headers=self._headers(),
            json={
                "files": [
                    {
                        "name": upload_name,
                        "data_id": non_empty(data_id, upload_name),
                        "is_ocr": is_ocr,
                    }
                ],
                "model_version": model_version,
                "enable_formula": enable_formula,
                "enable_table": enable_table,
                "language": language,
            },
            timeout_seconds=self._timeout_seconds,
        )
        upload = upload_batch_from_response(self._transport(apply_request), self._token)
        self._transport(
            MineruRequest(
                method="PUT",
                base_url=upload["upload_url"],
                path="",
                headers={},
                body=self._source_reader(source_url),
                timeout_seconds=self._timeout_seconds,
            )
        )
        return MineruTask(
            task_id=f"batch:{upload['batch_id']}",
            state="submitted",
            raw={
                "batch_id": upload["batch_id"],
                "file_name": upload_name,
                "data_id": non_empty(data_id, upload_name),
            },
        )

    def _get_extract_batch_task(self, batch_id: str) -> MineruTask:
        request = MineruRequest(
            method="GET",
            base_url=self._base_url,
            path=f"/api/v4/extract-results/batch/{batch_id}",
            headers=self._headers(),
            timeout_seconds=self._timeout_seconds,
        )
        return task_from_batch_response(self._transport(request), self._token, batch_id)

    def _headers(self) -> dict[str, str]:
        return {
            "Authorization": f"Bearer {self._token}",
            "Content-Type": "application/json",
        }


def urllib_transport(request: MineruRequest) -> Mapping[str, Any]:
    if request.method.upper() == "PUT" and request.body is not None and not request.headers:
        return put_bytes_no_content_type(
            request_url(request),
            request.body,
            timeout_seconds=request.timeout_seconds,
        )

    data = request.body
    if data is None and request.json is not None:
        data = json.dumps(request.json).encode("utf-8")
    try:
        with urlopen(
            Request(
                request_url(request),
                data=data,
                headers=dict(request.headers),
                method=request.method,
            ),
            timeout=request.timeout_seconds,
            context=default_ssl_context(),
        ) as response:
            body = response.read()
            if not body:
                return {}
            try:
                return json.loads(body.decode("utf-8"))
            except json.JSONDecodeError:
                return {"raw": body.decode("utf-8", errors="replace")}
    except HTTPError as error:
        detail = error.read().decode("utf-8", errors="replace").strip()
        suffix = f": {detail}" if detail else ""
        raise MineruError(f"MinerU HTTP {error.code}{suffix}") from error
    except URLError as error:
        raise MineruError(f"MinerU network error: {error.reason}") from error


def put_bytes_no_content_type(url: str, body: bytes, *, timeout_seconds: int) -> Mapping[str, Any]:
    parsed = urlparse(url)
    if parsed.scheme not in ("http", "https"):
        raise MineruError("MinerU upload URL must be HTTP or HTTPS")
    path = parsed.path or "/"
    if parsed.query:
        path = f"{path}?{parsed.query}"

    connection_cls = http.client.HTTPSConnection if parsed.scheme == "https" else http.client.HTTPConnection
    kwargs: dict[str, Any] = {"timeout": timeout_seconds}
    if parsed.scheme == "https":
        kwargs["context"] = default_ssl_context()
    connection = connection_cls(parsed.hostname, parsed.port, **kwargs)
    try:
        connection.putrequest("PUT", path, skip_accept_encoding=True)
        connection.putheader("Host", parsed.netloc)
        connection.putheader("Content-Length", str(len(body)))
        connection.endheaders(body)
        response = connection.getresponse()
        response_body = response.read()
        if response.status >= 400:
            detail = response_body.decode("utf-8", errors="replace").strip()
            suffix = f": {detail}" if detail else ""
            raise MineruError(f"MinerU upload HTTP {response.status}{suffix}")
        if not response_body:
            return {}
        try:
            return json.loads(response_body.decode("utf-8"))
        except json.JSONDecodeError:
            return {"raw": response_body.decode("utf-8", errors="replace")}
    finally:
        connection.close()


def request_url(request: MineruRequest) -> str:
    if not request.path:
        return request.base_url
    return urljoin(request.base_url, request.path)


def default_ssl_context() -> ssl.SSLContext:
    try:
        import certifi

        return ssl.create_default_context(cafile=certifi.where())
    except Exception:
        return ssl.create_default_context()


def download_source_bytes(source_url: str) -> bytes:
    parsed = urlparse(source_url)
    if parsed.scheme == "file":
        return Path(unquote(parsed.path)).read_bytes()
    with urlopen(source_url, timeout=120, context=default_ssl_context()) as response:
        return response.read()


def should_upload_source_url(source_url: str) -> bool:
    parsed = urlparse(source_url)
    if parsed.scheme == "file":
        return True
    host = (parsed.hostname or "").strip().lower()
    if host == "localhost":
        return True
    if not host:
        return False
    try:
        address = ipaddress.ip_address(host)
    except ValueError:
        return False
    return address.is_loopback or address.is_private or address.is_link_local


def file_name_from_url(source_url: str) -> str:
    parsed = urlparse(source_url)
    name = Path(unquote(parsed.path)).name.strip()
    return name or "document.pdf"


def non_empty(value: str, fallback: str) -> str:
    value = str(value or "").strip()
    return value or fallback


def response_data(response: Mapping[str, Any], token: str) -> Mapping[str, Any]:
    code = response.get("code")
    if code not in (0, "0"):
        msg = str(response.get("msg") or "MinerU request failed")
        raise MineruError(msg.replace(token, "[redacted]"))
    data = response.get("data")
    if not isinstance(data, Mapping):
        raise MineruError("MinerU response missing data")
    return data


def upload_batch_from_response(response: Mapping[str, Any], token: str) -> dict[str, str]:
    data = response_data(response, token)
    batch_id = str(data.get("batch_id") or data.get("batchId") or "")
    file_urls = data.get("file_urls") or data.get("fileUrls") or []
    if not batch_id:
        raise MineruError("MinerU upload response missing batch id")
    if not isinstance(file_urls, list) or not file_urls:
        raise MineruError("MinerU upload response missing file URL")
    first = file_urls[0]
    if isinstance(first, Mapping):
        upload_url = str(
            first.get("url")
            or first.get("upload_url")
            or first.get("uploadUrl")
            or first.get("file_url")
            or first.get("fileUrl")
            or ""
        )
    else:
        upload_url = str(first or "")
    if not upload_url:
        raise MineruError("MinerU upload response file URL is empty")
    return {"batch_id": batch_id, "upload_url": upload_url}


def task_from_response(response: Mapping[str, Any], token: str) -> MineruTask:
    data = response_data(response, token)

    task_id = str(data.get("task_id") or "")
    state = str(data.get("state") or data.get("status") or "")
    if not task_id:
        raise MineruError("MinerU response missing task id")
    if not state:
        state = "submitted"

    return MineruTask(
        task_id=task_id,
        state=state,
        full_zip_url=str(data.get("full_zip_url") or ""),
        err_msg=str(data.get("err_msg") or ""),
        raw=data,
    )


def task_from_batch_response(response: Mapping[str, Any], token: str, batch_id: str) -> MineruTask:
    data = response_data(response, token)
    results = data.get("extract_result") or data.get("extractResult") or []
    if not isinstance(results, list) or not results:
        return MineruTask(
            task_id=f"batch:{batch_id}",
            state="submitted",
            raw={"batch_id": batch_id},
        )
    first = results[0]
    if not isinstance(first, Mapping):
        raise MineruError("MinerU batch result item is invalid")
    state = str(first.get("state") or first.get("status") or "")
    if not state:
        state = "submitted"
    raw = dict(first)
    raw["batch_id"] = str(data.get("batch_id") or data.get("batchId") or batch_id)
    return MineruTask(
        task_id=f"batch:{batch_id}",
        state=state,
        full_zip_url=str(first.get("full_zip_url") or first.get("fullZipUrl") or ""),
        err_msg=str(first.get("err_msg") or first.get("errMsg") or ""),
        raw=raw,
    )
