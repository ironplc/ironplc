terraform {
  required_version = ">= 1.5.0"

  # State + plan/apply execution live in HCP Terraform. The org name is
  # read from the TF_CLOUD_ORGANIZATION environment variable so this
  # config is not tied to a specific user's HCP account — see
  # infrastructure/README.md for setup.
  cloud {
    workspaces {
      name = "ironplc-infrastructure"
    }
  }

  required_providers {
    github = {
      source  = "integrations/github"
      version = "~> 6.0"
    }
  }
}

provider "github" {
  token = var.github_token
  owner = var.github_owner
}

# ---------------------------------------------------------------------------
# Workflow labels. Colors are hex without the leading '#'.
# ---------------------------------------------------------------------------

resource "github_issue_label" "status_triage" {
  repository  = var.github_repo
  name        = "status/triage"
  color       = "ededed"
  description = "Awaiting initial triage by the agent."
}

resource "github_issue_label" "status_requirements" {
  repository  = var.github_repo
  name        = "status/requirements"
  color       = "0e8a16"
  description = "Requirements stage: a draft requirements comment has been posted."
}

resource "github_issue_label" "status_design" {
  repository  = var.github_repo
  name        = "status/design"
  color       = "1d76db"
  description = "Design stage: agent is producing or has posted a design."
}

resource "github_issue_label" "status_plan" {
  repository  = var.github_repo
  name        = "status/plan"
  color       = "5319e7"
  description = "Plan stage: agent is producing or has posted an implementation plan."
}

resource "github_issue_label" "status_code" {
  repository  = var.github_repo
  name        = "status/code"
  color       = "c5def5"
  description = "Code stage: agent is implementing the change."
}

resource "github_issue_label" "status_pr_open" {
  repository  = var.github_repo
  name        = "status/pr-open"
  color       = "0e8a16"
  description = "A pull request has been opened for this issue."
}

resource "github_issue_label" "status_closed" {
  repository  = var.github_repo
  name        = "status/closed"
  color       = "000000"
  description = "Work item completed and closed."
}

resource "github_issue_label" "status_needs_info" {
  repository  = var.github_repo
  name        = "status/needs-info"
  color       = "fbca04"
  description = "Agent needs more information from the reporter to proceed."
}

resource "github_issue_label" "review_requested" {
  repository  = var.github_repo
  name        = "review/requested"
  color       = "d4c5f9"
  description = "Maintainer review requested."
}

resource "github_issue_label" "review_approved" {
  repository  = var.github_repo
  name        = "review/approved"
  color       = "0e8a16"
  description = "Maintainer has approved the current artifact; advances the stage."
}

resource "github_issue_label" "review_changes_requested" {
  repository  = var.github_repo
  name        = "review/changes-requested"
  color       = "e99695"
  description = "Maintainer has requested changes; agent should revise."
}

resource "github_issue_label" "flag_agent_error" {
  repository  = var.github_repo
  name        = "flag/agent-error"
  color       = "d93f0b"
  description = "The agent failed; a maintainer should investigate."
}

resource "github_issue_label" "flag_revision_limit" {
  repository  = var.github_repo
  name        = "flag/revision-limit"
  color       = "b60205"
  description = "Stage hit the revision limit; maintainer intervention required."
}

resource "github_issue_label" "flag_blocked" {
  repository  = var.github_repo
  name        = "flag/blocked"
  color       = "b60205"
  description = "Blocked on an external dependency."
}

# ---------------------------------------------------------------------------
# Webhook pointing at the long-running orchestrator.
# ---------------------------------------------------------------------------

resource "github_repository_webhook" "agent" {
  repository = var.github_repo
  active     = true
  events     = ["issues", "issue_comment"]

  configuration {
    url          = var.webhook_url
    content_type = "json"
    insecure_ssl = false
    secret       = var.webhook_secret
  }
}
