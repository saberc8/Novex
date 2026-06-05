from dataclasses import dataclass
from typing import Any, Callable, Mapping
from urllib.error import HTTPError, URLError
from urllib.parse import urljoin
from urllib.request import Request, urlopen
import json


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
    timeout_seconds: int = 120


@dataclass(frozen=True)
class MineruTask:
    task_id: str
    state: str
    full_zip_url: str = ""
    err_msg: str = ""
    raw: Mapping[str, Any] | None = None


MineruTransport = Callable[[MineruRequest], Mapping[str, Any]]


class MineruClient:
    def __init__(
        self,
        token: str,
        base_url: str = DEFAULT_MINERU_BASE_URL,
        timeout_seconds: int = 120,
        transport: MineruTransport | None = None,
    ) -> None:
        token = token.strip()
        if not token:
            raise MineruError("MINERU_TOKEN is required")
        self._token = token
        self._base_url = base_url.rstrip("/")
        self._timeout_seconds = timeout_seconds
        self._transport = transport or urllib_transport

    def create_extract_task(
        self,
        source_url: str,
        *,
        model_version: str = DEFAULT_MINERU_MODEL_VERSION,
        is_ocr: bool = False,
        enable_formula: bool = True,
        enable_table: bool = True,
        language: str = DEFAULT_MINERU_LANGUAGE,
    ) -> MineruTask:
        source_url = source_url.strip()
        if not source_url:
            raise MineruError("source url is required")

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

        request = MineruRequest(
            method="GET",
            base_url=self._base_url,
            path=f"/api/v4/extract/task/{task_id}",
            headers=self._headers(),
            timeout_seconds=self._timeout_seconds,
        )
        return task_from_response(self._transport(request), self._token)

    def _headers(self) -> dict[str, str]:
        return {
            "Authorization": f"Bearer {self._token}",
            "Content-Type": "application/json",
        }


def urllib_transport(request: MineruRequest) -> Mapping[str, Any]:
    data = None
    if request.json is not None:
        data = json.dumps(request.json).encode("utf-8")
    try:
        with urlopen(
            Request(
                urljoin(request.base_url, request.path),
                data=data,
                headers=dict(request.headers),
                method=request.method,
            ),
            timeout=request.timeout_seconds,
        ) as response:
            return json.loads(response.read().decode("utf-8"))
    except HTTPError as error:
        raise MineruError(f"MinerU HTTP {error.code}") from error
    except URLError as error:
        raise MineruError(f"MinerU network error: {error.reason}") from error


def task_from_response(response: Mapping[str, Any], token: str) -> MineruTask:
    code = response.get("code")
    if code not in (0, "0"):
        msg = str(response.get("msg") or "MinerU request failed")
        raise MineruError(msg.replace(token, "[redacted]"))

    data = response.get("data")
    if not isinstance(data, Mapping):
        raise MineruError("MinerU response missing data")

    task_id = str(data.get("task_id") or "")
    state = str(data.get("state") or "")
    if not task_id or not state:
        raise MineruError("MinerU response missing task state")

    return MineruTask(
        task_id=task_id,
        state=state,
        full_zip_url=str(data.get("full_zip_url") or ""),
        err_msg=str(data.get("err_msg") or ""),
        raw=data,
    )
