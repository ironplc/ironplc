variable "github_token" {
  description = "GitHub fine-grained PAT with admin:webhooks + issues:write on the repo."
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

variable "webhook_url" {
  description = "Public HTTPS URL where the orchestrator receives events (ngrok or prod)."
  type        = string
}

variable "webhook_secret" {
  description = "Shared HMAC secret; must match GITHUB_WEBHOOK_SECRET in the app .env."
  type        = string
  sensitive   = true
}
