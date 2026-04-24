output "webhook_url" {
  description = "Public URL the GitHub webhook delivers to."
  value       = var.webhook_url
}

output "webhook_id" {
  description = "GitHub-assigned webhook id (useful for manual redelivery)."
  value       = github_repository_webhook.agent.id
}
