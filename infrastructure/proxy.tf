# ---------------------------------------------------------------------------
# PostHog managed reverse proxy.
#
# WHY: The website and playground load PostHog from us.i.posthog.com directly
# (see docs/_static/posthog-init.js and playground/posthog-init.js). Ad
# blockers recognise those hosts from their filter lists and drop the requests,
# so a slice of pageviews and product events never arrive. Routing analytics
# through a FIRST-PARTY subdomain fixes that — the traffic looks like requests
# to our own site.
#
# HOW: `posthog_proxy_record` uses PostHog's *managed* reverse proxy: PostHog
# runs the proxy and terminates TLS on our subdomain. We only:
#
#   1. `terraform apply` this resource — it returns `target_cname` (below).
#   2. Create a DNS CNAME: posthog_proxy_domain -> target_cname.
#   3. Wait for PostHog to verify the CNAME and provision the certificate
#      (status converges to "valid" OUTSIDE Terraform; a later `apply` or
#      `terraform refresh` will show it).
#
# The resource is IMMUTABLE — changing the domain replaces it (and mints a new
# target_cname, so the CNAME must be repointed).
#
# NOTE: pick a neutral domain. Filter lists block hostnames containing
# "analytics", "track", "telemetry", "posthog", "stats" — using one of those
# would defeat the whole point. The variable defaults to "hog.ironplc.com".
#
# FOLLOW-UP (NOT done here — the CNAME must exist and resolve first, or events
# would 404 until DNS converges): once `target_cname` is live, repoint the two
# SDK loaders at the proxy so events actually flow through it:
#   - docs/_static/posthog-init.js  -> api_host: "https://<posthog_proxy_domain>"
#   - playground/posthog-init.js    -> api_host: "https://<posthog_proxy_domain>"
#     add ui_host: "https://us.posthog.com" so toolbar/links still point at the
#     PostHog app.
# ---------------------------------------------------------------------------

resource "posthog_proxy_record" "ingest" {
  organization_id = var.posthog_organization_id
  domain          = var.posthog_proxy_domain
}

# The PostHog-managed CNAME target. Create a DNS CNAME record pointing
# var.posthog_proxy_domain at this value, then let PostHog verify it.
output "posthog_proxy_target_cname" {
  description = "Point a DNS CNAME for var.posthog_proxy_domain at this value."
  value       = posthog_proxy_record.ingest.target_cname
}

output "posthog_proxy_status" {
  description = "PostHog provisioning status of the reverse-proxy record (converges to \"valid\" once the CNAME resolves)."
  value       = posthog_proxy_record.ingest.status
}
