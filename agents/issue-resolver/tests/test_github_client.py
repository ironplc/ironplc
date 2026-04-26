"""Tests for github_client signature verification + basic client behavior."""

from __future__ import annotations

import hmac
import unittest
from hashlib import sha256
from unittest.mock import MagicMock, patch

from github_client import (
    DryRunGitHubClient,
    GitHubAPIError,
    GitHubClient,
    verify_webhook_signature,
)


class TestVerifyWebhookSignature(unittest.TestCase):
    def test_verify_when_valid_signature_then_true(self) -> None:
        secret = "topsecret"
        body = b'{"hello":"world"}'
        digest = hmac.new(secret.encode(), body, sha256).hexdigest()
        self.assertTrue(
            verify_webhook_signature(secret, body, f"sha256={digest}")
        )

    def test_verify_when_wrong_signature_then_false(self) -> None:
        self.assertFalse(
            verify_webhook_signature("secret", b"body", "sha256=deadbeef")
        )

    def test_verify_when_header_missing_then_false(self) -> None:
        self.assertFalse(verify_webhook_signature("secret", b"body", None))

    def test_verify_when_wrong_prefix_then_false(self) -> None:
        self.assertFalse(
            verify_webhook_signature("secret", b"body", "sha1=deadbeef")
        )

    def test_verify_when_body_differs_then_false(self) -> None:
        secret = "topsecret"
        digest = hmac.new(secret.encode(), b"original", sha256).hexdigest()
        self.assertFalse(
            verify_webhook_signature(secret, b"tampered", f"sha256={digest}")
        )


class TestGitHubClient(unittest.TestCase):
    @patch("github_client.requests.request")
    def test_get_issue_when_ok_then_returns_json(self, mock_req: MagicMock) -> None:
        resp = MagicMock()
        resp.status_code = 200
        resp.json.return_value = {"number": 42, "title": "x"}
        mock_req.return_value = resp

        client = GitHubClient(token="tok", repo="o/r")
        data = client.get_issue(42)
        self.assertEqual(data["number"], 42)

    @patch("github_client.time.sleep", lambda *_a, **_k: None)
    @patch("github_client.requests.request")
    def test_request_when_500_then_retries_once(self, mock_req: MagicMock) -> None:
        bad = MagicMock(status_code=500, text="boom")
        good = MagicMock(status_code=200)
        good.json.return_value = {"ok": True}
        mock_req.side_effect = [bad, good]

        client = GitHubClient(token="tok", repo="o/r")
        client.get_issue(1)
        self.assertEqual(mock_req.call_count, 2)

    @patch("github_client.requests.request")
    def test_request_when_4xx_then_raises(self, mock_req: MagicMock) -> None:
        resp = MagicMock(status_code=403, text="forbidden")
        mock_req.return_value = resp
        client = GitHubClient(token="tok", repo="o/r")
        with self.assertRaises(GitHubAPIError) as ctx:
            client.get_issue(1)
        self.assertEqual(ctx.exception.status, 403)

    @patch("github_client.requests.request")
    def test_remove_label_when_404_then_silent(self, mock_req: MagicMock) -> None:
        resp = MagicMock(status_code=404, text="not found")
        mock_req.return_value = resp
        client = GitHubClient(token="tok", repo="o/r")
        # Must not raise.
        client.remove_label(42, "status/triage")


class TestDryRunGitHubClient(unittest.TestCase):
    def test_post_comment_when_dry_run_then_does_not_call_inner(self) -> None:
        inner = MagicMock(spec=GitHubClient)
        client = DryRunGitHubClient(inner)
        result = client.post_comment(7, "hello")
        inner.post_comment.assert_not_called()
        self.assertEqual(result, {"id": 0})

    def test_label_writes_when_dry_run_then_does_not_call_inner(self) -> None:
        inner = MagicMock(spec=GitHubClient)
        client = DryRunGitHubClient(inner)
        client.add_label(7, "status/triage")
        client.remove_label(7, "status/triage")
        inner.add_label.assert_not_called()
        inner.remove_label.assert_not_called()

    def test_reads_when_dry_run_then_delegate_to_inner(self) -> None:
        inner = MagicMock(spec=GitHubClient)
        inner.get_issue.return_value = {"number": 7}
        inner.get_issue_comments.return_value = [{"id": 1}]
        inner.get_labels.return_value = ["status/triage"]
        inner.get_authenticated_user_login.return_value = "bot"
        client = DryRunGitHubClient(inner)

        self.assertEqual(client.get_issue(7), {"number": 7})
        self.assertEqual(client.get_issue_comments(7), [{"id": 1}])
        self.assertEqual(client.get_labels(7), ["status/triage"])
        self.assertEqual(client.get_authenticated_user_login(), "bot")


if __name__ == "__main__":
    unittest.main()
