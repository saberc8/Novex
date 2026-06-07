import os
import unittest

from parser_worker.config import load_config
from parser_worker.mineru_client import MineruClient


LIVE_MINERU_PDF_URL = "https://www.w3.org/WAI/ER/tests/xhtml/testfiles/resources/pdf/dummy.pdf"


@unittest.skipUnless(
    os.environ.get("NOVEX_LIVE_MINERU_TEST") == "1",
    "set NOVEX_LIVE_MINERU_TEST=1 to submit a real MinerU task",
)
class MineruLiveSmokeTest(unittest.TestCase):
    def test_live_mineru_submit_extract_task_accepts_configured_token(self):
        config = load_config()
        self.assertTrue(config.mineru.configured, "MINERU_TOKEN must be configured")
        client = MineruClient(
            token=config.mineru.token,
            timeout_seconds=config.mineru.timeout_seconds,
        )
        task = client.create_extract_task(
            os.environ.get("NOVEX_LIVE_MINERU_PDF_URL", LIVE_MINERU_PDF_URL),
            enable_formula=False,
            enable_table=True,
        )

        self.assertTrue(task.task_id)
        self.assertTrue(task.state)


if __name__ == "__main__":
    unittest.main()
