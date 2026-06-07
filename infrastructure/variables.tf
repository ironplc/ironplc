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
