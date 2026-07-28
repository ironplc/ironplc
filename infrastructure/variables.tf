variable "github_token" {
  description = "GitHub fine-grained PAT with issues:write on the repo."
  type        = string
  sensitive   = true
}

variable "github_owner" {
  description = "GitHub org or user that owns the repo (e.g. ironplc)."
  type        = string
}

variable "github_repo" {
  description = "Repository name without the owner prefix (e.g. ironplc)."
  type        = string
}

variable "posthog_api_key" {
  description = "PostHog personal API key with insight:write + dashboard:write scopes. NOT the public phc_ ingestion key."
  type        = string
  sensitive   = true
}

variable "posthog_project_id" {
  description = "PostHog project (environment) numeric ID, as a string. Found in Project Settings."
  type        = string
}

variable "posthog_host" {
  description = "PostHog app host for the API (region-specific). Not the ingestion host."
  type        = string
  default     = "https://us.posthog.com"
}
