"""Environment-backed configuration for the issue-resolver orchestrator.

Loads required secrets and settings from a local `.env` file via
python-dotenv, then validates that every required variable is set.
Missing values raise :class:`ConfigError` listing *all* missing names at
once so a misconfigured deployment surfaces every problem on the first
boot attempt rather than one at a time.

The loaded values are never logged. Callers should pass the ``Config``
instance directly to clients that need secrets.
"""

from __future__ import annotations

import os
from dataclasses import dataclass

from dotenv import load_dotenv


class ConfigError(RuntimeError):
    """Raised when required configuration is missing or invalid."""


@dataclass(frozen=True)
class Config:
    anthropic_api_key: str
    github_token: str
    github_webhook_secret: str
    github_repo: str


_REQUIRED = (
    "ANTHROPIC_API_KEY",
    "GITHUB_TOKEN",
    "GITHUB_WEBHOOK_SECRET",
    "GITHUB_REPO",
)


def load_config(env_file: str | None = None) -> Config:
    """Load configuration from environment (and optional ``.env`` file).

    Raises ``ConfigError`` listing every missing variable — never logs
    secret values.
    """
    if env_file is not None:
        load_dotenv(env_file)
    else:
        load_dotenv()

    missing = [name for name in _REQUIRED if not os.environ.get(name)]
    if missing:
        raise ConfigError(
            "Missing required environment variables: " + ", ".join(missing)
        )

    return Config(
        anthropic_api_key=os.environ["ANTHROPIC_API_KEY"],
        github_token=os.environ["GITHUB_TOKEN"],
        github_webhook_secret=os.environ["GITHUB_WEBHOOK_SECRET"],
        github_repo=os.environ["GITHUB_REPO"],
    )
