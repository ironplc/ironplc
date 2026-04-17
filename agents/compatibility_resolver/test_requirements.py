"""Unit tests for agents/compatibility_resolver/requirements.py.

All network calls are mocked — no real HTTP or Anthropic API calls.
"""

import json
import unittest
from unittest.mock import MagicMock, patch, call

from requirements import (
    build_validation_prompt,
    parse_validation_response,
    build_requirements_prompt,
    format_comment,
    ensure_label_exists,
    main,
)


# ---------------------------------------------------------------------------
# build_validation_prompt
# ---------------------------------------------------------------------------


class TestBuildValidationPrompt(unittest.TestCase):
    def test_build_validation_prompt_when_title_and_body_then_includes_both(self):
        result = build_validation_prompt("My title", "My body text")
        self.assertIn("My title", result)
        self.assertIn("My body text", result)

    def test_build_validation_prompt_when_body_empty_then_includes_title_only(self):
        result = build_validation_prompt("My title", "")
        self.assertIn("My title", result)
        # Should not include the "Issue body:" prefix for empty body
        self.assertNotIn("Issue body:", result)


# ---------------------------------------------------------------------------
# parse_validation_response
# ---------------------------------------------------------------------------


class TestParseValidationResponse(unittest.TestCase):
    def test_parse_validation_response_when_sufficient_true_then_returns_valid(self):
        text = json.dumps({"sufficient": True})
        sufficient, missing = parse_validation_response(text)
        self.assertTrue(sufficient)
        self.assertIsNone(missing)

    def test_parse_validation_response_when_sufficient_false_then_returns_missing_reason(self):
        text = json.dumps({"sufficient": False, "missing": "No ST program provided"})
        sufficient, missing = parse_validation_response(text)
        self.assertFalse(sufficient)
        self.assertEqual(missing, "No ST program provided")

    def test_parse_validation_response_when_malformed_json_then_returns_invalid(self):
        sufficient, missing = parse_validation_response("not json at all")
        self.assertFalse(sufficient)
        self.assertIn("Unable to parse", missing)


# ---------------------------------------------------------------------------
# build_requirements_prompt
# ---------------------------------------------------------------------------


class TestBuildRequirementsPrompt(unittest.TestCase):
    def test_build_requirements_prompt_when_called_then_includes_issue_body(self):
        result = build_requirements_prompt("title", "my issue body")
        self.assertIn("my issue body", result)

    def test_build_requirements_prompt_when_called_then_includes_title(self):
        result = build_requirements_prompt("my title", "body")
        self.assertIn("my title", result)


# ---------------------------------------------------------------------------
# format_comment
# ---------------------------------------------------------------------------


class TestFormatComment(unittest.TestCase):
    def test_format_comment_when_content_has_open_questions_then_no_append(self):
        content = "**REQ-CG-001** ...\n\n## Open Questions\n\n- Something?"
        result = format_comment(content)
        self.assertIn("Auto-generated requirements", result)
        # Should not double-add an Open Questions section
        self.assertEqual(result.count("Open Questions"), 1)

    def test_format_comment_when_content_missing_open_questions_then_appends_section(self):
        content = "**REQ-CG-001** The system SHALL support XYZ."
        result = format_comment(content)
        self.assertIn("Open Questions", result)
        self.assertIn("None identified", result)


# ---------------------------------------------------------------------------
# ensure_label_exists
# ---------------------------------------------------------------------------


class TestEnsureLabelExists(unittest.TestCase):
    @patch("requirements.requests")
    def test_ensure_label_exists_when_label_exists_then_no_create_call(self, mock_requests):
        mock_resp = MagicMock()
        mock_resp.status_code = 200
        mock_requests.get.return_value = mock_resp

        ensure_label_exists("owner/repo", "status/requirements", "fake-token")

        mock_requests.get.assert_called_once()
        mock_requests.post.assert_not_called()

    @patch("requirements.requests")
    def test_ensure_label_exists_when_label_missing_then_creates_with_color(self, mock_requests):
        get_resp = MagicMock()
        get_resp.status_code = 404
        mock_requests.get.return_value = get_resp

        post_resp = MagicMock()
        post_resp.raise_for_status = MagicMock()
        mock_requests.post.return_value = post_resp

        ensure_label_exists("owner/repo", "status/requirements", "fake-token")

        mock_requests.post.assert_called_once()
        post_call_kwargs = mock_requests.post.call_args
        self.assertEqual(post_call_kwargs.kwargs["json"]["name"], "status/requirements")
        self.assertEqual(post_call_kwargs.kwargs["json"]["color"], "0e8a16")


# ---------------------------------------------------------------------------
# main integration
# ---------------------------------------------------------------------------


def _mock_anthropic_message(text):
    """Create a mock Anthropic message response."""
    msg = MagicMock()
    block = MagicMock()
    block.text = text
    msg.content = [block]
    return msg


class TestMain(unittest.TestCase):
    @patch.dict("os.environ", {
        "GITHUB_TOKEN": "gh-token",
        "ANTHROPIC_API_KEY": "ant-key",
        "ISSUE_NUMBER": "42",
        "GITHUB_REPOSITORY": "ironplc/ironplc",
    })
    @patch("requirements.requests")
    @patch("requirements.anthropic.Anthropic")
    def test_main_when_valid_issue_then_posts_requirements_comment(
        self, MockAnthropic, mock_requests
    ):
        # Mock GitHub GET issue
        issue_resp = MagicMock()
        issue_resp.json.return_value = {
            "title": "MOD operator not supported",
            "body": "```st\nVAR x: INT; END_VAR\nx := 10 MOD 3;\n```\n"
                    "**IronPLC Behavior**: Error\n**Expected**: x = 1",
        }
        issue_resp.raise_for_status = MagicMock()

        # Mock label GET (exists)
        label_resp = MagicMock()
        label_resp.status_code = 200

        mock_requests.get.side_effect = [issue_resp, label_resp, label_resp]

        # Mock POST (comment, add label, remove label)
        post_resp = MagicMock()
        post_resp.raise_for_status = MagicMock()
        mock_requests.post.return_value = post_resp

        delete_resp = MagicMock()
        delete_resp.status_code = 200
        delete_resp.raise_for_status = MagicMock()
        mock_requests.delete.return_value = delete_resp

        # Mock Anthropic: validation + requirements
        client = MockAnthropic.return_value
        client.messages.create.side_effect = [
            _mock_anthropic_message('{"sufficient": true}'),
            _mock_anthropic_message(
                "**REQ-CG-001** The system SHALL support the MOD operator.\n\n"
                "## Open Questions\n\n- None"
            ),
        ]

        main()

        # Two Anthropic calls: validation + generation
        self.assertEqual(client.messages.create.call_count, 2)
        # Comment was posted (first POST call)
        post_calls = mock_requests.post.call_args_list
        comment_call = post_calls[0]
        self.assertIn("comments", comment_call.args[0])
        self.assertIn("Auto-generated requirements", comment_call.kwargs["json"]["body"])

    @patch.dict("os.environ", {
        "GITHUB_TOKEN": "gh-token",
        "ANTHROPIC_API_KEY": "ant-key",
        "ISSUE_NUMBER": "42",
        "GITHUB_REPOSITORY": "ironplc/ironplc",
    })
    @patch("requirements.requests")
    @patch("requirements.anthropic.Anthropic")
    def test_main_when_insufficient_info_then_posts_needs_info_comment(
        self, MockAnthropic, mock_requests
    ):
        # Mock GitHub GET issue
        issue_resp = MagicMock()
        issue_resp.json.return_value = {
            "title": "Something doesn't work",
            "body": "It's broken.",
        }
        issue_resp.raise_for_status = MagicMock()

        label_resp = MagicMock()
        label_resp.status_code = 200

        mock_requests.get.side_effect = [issue_resp, label_resp]

        post_resp = MagicMock()
        post_resp.raise_for_status = MagicMock()
        mock_requests.post.return_value = post_resp

        delete_resp = MagicMock()
        delete_resp.status_code = 200
        delete_resp.raise_for_status = MagicMock()
        mock_requests.delete.return_value = delete_resp

        # Mock Anthropic: validation says insufficient
        client = MockAnthropic.return_value
        client.messages.create.return_value = _mock_anthropic_message(
            '{"sufficient": false, "missing": "No ST program provided"}'
        )

        main()

        # Only ONE Anthropic call (validation only)
        self.assertEqual(client.messages.create.call_count, 1)
        # Comment mentions what's missing
        post_calls = mock_requests.post.call_args_list
        comment_call = post_calls[0]
        self.assertIn("No ST program provided", comment_call.kwargs["json"]["body"])

    @patch.dict("os.environ", {
        "GITHUB_TOKEN": "gh-token",
        "ANTHROPIC_API_KEY": "ant-key",
        "ISSUE_NUMBER": "42",
        "GITHUB_REPOSITORY": "ironplc/ironplc",
    })
    @patch("requirements.requests")
    @patch("requirements.anthropic.Anthropic")
    def test_main_when_anthropic_fails_then_posts_error_comment(
        self, MockAnthropic, mock_requests
    ):
        # Mock GitHub GET issue
        issue_resp = MagicMock()
        issue_resp.json.return_value = {
            "title": "MOD not supported",
            "body": "Some body",
        }
        issue_resp.raise_for_status = MagicMock()
        mock_requests.get.side_effect = [issue_resp, MagicMock(status_code=200)]

        post_resp = MagicMock()
        post_resp.raise_for_status = MagicMock()
        mock_requests.post.return_value = post_resp

        # Mock Anthropic: raise exception
        client = MockAnthropic.return_value
        client.messages.create.side_effect = Exception("API rate limited")

        main()

        # Error comment was posted
        post_calls = mock_requests.post.call_args_list
        comment_call = post_calls[0]
        self.assertIn("failed", comment_call.kwargs["json"]["body"].lower())
        # flag/agent-error label added
        label_calls = [
            c for c in post_calls
            if "labels" in c.args[0]
        ]
        self.assertTrue(len(label_calls) > 0)


if __name__ == "__main__":
    unittest.main()
