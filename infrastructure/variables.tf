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

variable "posthog_proxy_domain" {
  description = <<-EOT
    Custom domain for PostHog's managed reverse proxy (see proxy.tf).
    Routing analytics through a first-party subdomain is what stops ad
    blockers dropping events. Pick a NEUTRAL name: filter lists (EasyPrivacy,
    uBlock) block hostnames containing "analytics", "track", "telemetry",
    "posthog", "stats", so a name like analytics.ironplc.com would defeat the
    purpose. Default "hog.ironplc.com" is safe.
  EOT
  type        = string
  default     = "hog.ironplc.com"
}

variable "posthog_organization_id" {
  description = <<-EOT
    Organization that owns the managed reverse-proxy record. Accepts a UUID,
    an org slug, or "@current" for the API key's organization. The proxy
    record is org-scoped (unlike the project-scoped dashboard/insights).
  EOT
  type        = string
  default     = "@current"
}
