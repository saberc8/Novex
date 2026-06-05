import os
import unittest

from parser_worker.config import load_config, mask_secret
from parser_worker.health import health_summary


class ParserWorkerConfigTest(unittest.TestCase):
    def test_loads_mineru_token_from_environment(self):
        env = {
            "MINERU_TOKEN": "mineru-token-1234567890",
            "PARSER_WORKER_MODE": "mineru",
        }

        config = load_config(env)

        self.assertEqual(config.mode, "mineru")
        self.assertTrue(config.mineru.configured)
        self.assertEqual(config.mineru.masked_token, "mine****7890")

    def test_masks_short_and_empty_secrets_without_leaking_values(self):
        self.assertEqual(mask_secret(""), "")
        self.assertEqual(mask_secret("abc"), "***")
        self.assertEqual(mask_secret("abcdefghi"), "abcd****fghi")

    def test_health_summary_never_exposes_raw_mineru_token(self):
        env = {"MINERU_TOKEN": "mineru-secret-value"}

        summary = health_summary(load_config(env))

        self.assertTrue(summary["mineru"]["configured"])
        self.assertEqual(summary["mineru"]["token"], "mine****alue")
        self.assertNotIn("mineru-secret-value", str(summary))

    def test_missing_mineru_token_reports_unconfigured_worker(self):
        env = dict(os.environ)
        env.pop("MINERU_TOKEN", None)

        summary = health_summary(load_config(env))

        self.assertFalse(summary["mineru"]["configured"])
        self.assertEqual(summary["mineru"]["token"], "")


if __name__ == "__main__":
    unittest.main()
