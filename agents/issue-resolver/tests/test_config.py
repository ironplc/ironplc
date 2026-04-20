"""Tests for config.load_config."""

from __future__ import annotations

import os
import unittest
from unittest.mock import patch

from config import ConfigError, load_config


class TestLoadConfig(unittest.TestCase):
    def setUp(self) -> None:
        self._saved_env = {
            k: os.environ.pop(k, None)
            for k in (
                "ANTHROPIC_API_KEY",
                "GITHUB_TOKEN",
                "GITHUB_WEBHOOK_SECRET",
                "GITHUB_REPO",
            )
        }

    def tearDown(self) -> None:
        for k, v in self._saved_env.items():
            if v is not None:
                os.environ[k] = v
            else:
                os.environ.pop(k, None)

    @patch("config.load_dotenv", lambda *_a, **_k: None)
    def test_load_config_when_all_env_set_then_returns_config(self) -> None:
        os.environ.update(
            {
                "ANTHROPIC_API_KEY": "ak",
                "GITHUB_TOKEN": "gh",
                "GITHUB_WEBHOOK_SECRET": "ws",
                "GITHUB_REPO": "o/r",
            }
        )
        config = load_config()
        self.assertEqual(config.anthropic_api_key, "ak")
        self.assertEqual(config.github_token, "gh")
        self.assertEqual(config.github_webhook_secret, "ws")
        self.assertEqual(config.github_repo, "o/r")

    @patch("config.load_dotenv", lambda *_a, **_k: None)
    def test_load_config_when_missing_then_lists_all_missing(self) -> None:
        os.environ["ANTHROPIC_API_KEY"] = "ak"
        # The rest are intentionally absent.
        with self.assertRaises(ConfigError) as ctx:
            load_config()
        message = str(ctx.exception)
        self.assertIn("GITHUB_TOKEN", message)
        self.assertIn("GITHUB_WEBHOOK_SECRET", message)
        self.assertIn("GITHUB_REPO", message)


if __name__ == "__main__":
    unittest.main()
