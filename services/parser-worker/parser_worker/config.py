from dataclasses import dataclass
from os import environ
from typing import Mapping


DEFAULT_WORKER_MODE = "type-routed"
DEFAULT_MINERU_TIMEOUT_SECONDS = 120


@dataclass(frozen=True)
class MineruConfig:
    token: str
    timeout_seconds: int

    @property
    def configured(self) -> bool:
        return bool(self.token)

    @property
    def masked_token(self) -> str:
        return mask_secret(self.token)


@dataclass(frozen=True)
class ParserWorkerConfig:
    mode: str
    mineru: MineruConfig


def load_config(env: Mapping[str, str] | None = None) -> ParserWorkerConfig:
    values = env if env is not None else environ
    return ParserWorkerConfig(
        mode=non_empty(values.get("PARSER_WORKER_MODE"), DEFAULT_WORKER_MODE),
        mineru=MineruConfig(
            token=non_empty(values.get("MINERU_TOKEN"), ""),
            timeout_seconds=parse_positive_int(
                values.get("MINERU_TIMEOUT_SECONDS"),
                DEFAULT_MINERU_TIMEOUT_SECONDS,
            ),
        ),
    )


def mask_secret(value: str) -> str:
    if not value:
        return ""
    if len(value) <= 4:
        return "***"
    if len(value) <= 8:
        return f"{value[:2]}****{value[-2:]}"
    return f"{value[:4]}****{value[-4:]}"


def non_empty(value: str | None, default: str) -> str:
    if value is None:
        return default
    value = value.strip()
    return value or default


def parse_positive_int(value: str | None, default: int) -> int:
    if value is None or not value.strip():
        return default
    parsed = int(value)
    if parsed <= 0:
        raise ValueError("value must be positive")
    return parsed
