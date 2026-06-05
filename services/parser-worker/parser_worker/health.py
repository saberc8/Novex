import json

from parser_worker.config import ParserWorkerConfig, load_config


def health_summary(config: ParserWorkerConfig | None = None) -> dict:
    config = config or load_config()
    return {
        "service": "parser-worker",
        "mode": config.mode,
        "mineru": {
            "configured": config.mineru.configured,
            "token": config.mineru.masked_token,
            "timeoutSeconds": config.mineru.timeout_seconds,
        },
    }


def main() -> None:
    print(json.dumps(health_summary(), ensure_ascii=False, sort_keys=True))


if __name__ == "__main__":
    main()
