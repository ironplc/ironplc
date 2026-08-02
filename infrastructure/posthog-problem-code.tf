# ---------------------------------------------------------------------------
# PostHog "Problem-code reach" dashboard.
#
# Problem-code documentation pages (…/reference/<section>/problems/<CODE>.html)
# are linked from every IronPLC surface: the playground, the CLI, the editor
# extension, and the MCP server. Each link carries two plain query params added
# by the client:
#   - channel = playground | cli | extension | mcp   (the origin)
#   - version = <client version>                      (also drives the
#                                                      out-of-date banner in
#                                                      docs/_static/version-check.js)
#
# These are deliberately NOT utm_* names (which read as tracking and are stripped
# by ad-blockers). docs/_static/posthog-init.js registers them via
# `custom_campaign_params`, so PostHog captures `channel` and `version` as event
# properties exactly like the built-in utm_* params — no per-insight mapping.
#
# The docs site emits $pageview on every page (docs/_static/posthog-init.js), so
# these tiles re-cut that existing $pageview stream — no new instrumentation — to
# answer "from where, and on which version, do people reach our problem-code
# docs?". Views without a `channel` (organic docs navigation) surface as the
# breakdown's null bucket, so each channel is visible alongside organic traffic.
#
# Cardinality note: the LSP/CLI paths also append &file/&line. Those never
# fragment these tiles because every insight breaks down on a low-cardinality
# property (channel, version, $pathname, $referring_domain), never the full URL.
#
# NOTE: query_json field values (property-filter operators such as `is_set`,
# breakdownFilter shape, display enums) can vary by PostHog version. Validate
# with `terraform plan` / `terraform apply` and adjust anything the API rejects.
# ---------------------------------------------------------------------------

locals {
  ph_problem_tags = ["managed-by-terraform", "problem-code-reach"]

  # Scope: any view of a problem-code doc page. Applied to every tile; the
  # channel breakdown then splits client-link arrivals from organic (null).
  problem_code_path = [
    { key = "$pathname", type = "event", operator = "icontains", value = ["/problems/"] },
  ]
}

resource "posthog_dashboard" "problem_code_reach" {
  name        = "IronPLC — Problem-code reach"
  description = "Where (and on which version) people reach problem-code docs, by channel: playground / cli / extension / mcp. Managed by Terraform (infrastructure/posthog-problem-code.tf)."
  pinned      = true
  tags        = local.ph_problem_tags
}

# ---------------------------------------------------------------------------
# Headline
# ---------------------------------------------------------------------------

resource "posthog_insight" "problem_code_total_arrivals" {
  name          = "Problem-code arrivals"
  description   = "Total problem-code page views over the trailing window (all sources, organic and client links)."
  dashboard_ids = [posthog_dashboard.problem_code_reach.id]
  tags          = local.ph_problem_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind = "TrendsQuery"
      series = [{
        kind       = "EventsNode"
        event      = "$pageview"
        name       = "$pageview"
        math       = "total"
        properties = local.problem_code_path
      }]
      interval     = "week"
      dateRange    = { date_from = local.ph_date_from }
      trendsFilter = { display = "BoldNumber" }
    }
  })
}

resource "posthog_insight" "problem_code_reach_by_channel" {
  name          = "Reach by channel"
  description   = "Unique visitors reaching problem-code docs, broken down by channel (playground / cli / extension / mcp). Organic docs navigation appears as the null bucket. The headline adoption-by-channel tile."
  dashboard_ids = [posthog_dashboard.problem_code_reach.id]
  tags          = local.ph_problem_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind = "TrendsQuery"
      series = [{
        kind       = "EventsNode"
        event      = "$pageview"
        name       = "$pageview"
        math       = "dau"
        properties = local.problem_code_path
      }]
      interval        = "week"
      dateRange       = { date_from = local.ph_date_from }
      breakdownFilter = { breakdowns = [{ property = "channel", type = "event" }] }
      trendsFilter    = { display = "ActionsTable" }
    }
  })
}

resource "posthog_insight" "problem_code_channel_trend" {
  name          = "Channel trend"
  description   = "Weekly unique visitors reaching problem-code docs per channel, over time."
  dashboard_ids = [posthog_dashboard.problem_code_reach.id]
  tags          = local.ph_problem_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind = "TrendsQuery"
      series = [{
        kind       = "EventsNode"
        event      = "$pageview"
        name       = "$pageview"
        math       = "dau"
        properties = local.problem_code_path
      }]
      interval        = "week"
      dateRange       = { date_from = local.ph_date_from }
      breakdownFilter = { breakdowns = [{ property = "channel", type = "event" }] }
      trendsFilter    = { display = "ActionsLineGraph" }
    }
  })
}

# ---------------------------------------------------------------------------
# What and who
# ---------------------------------------------------------------------------

resource "posthog_insight" "problem_code_top_codes" {
  name          = "Top problem codes"
  description   = "Most-reached problem-code pages, by path. Reveals which diagnostics people actually open the docs for."
  dashboard_ids = [posthog_dashboard.problem_code_reach.id]
  tags          = local.ph_problem_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind = "TrendsQuery"
      series = [{
        kind       = "EventsNode"
        event      = "$pageview"
        name       = "$pageview"
        math       = "total"
        properties = local.problem_code_path
      }]
      interval        = "week"
      dateRange       = { date_from = local.ph_date_from }
      breakdownFilter = { breakdowns = [{ property = "$pathname", type = "event" }] }
      trendsFilter    = { display = "ActionsBarValue" }
    }
  })
}

resource "posthog_insight" "problem_code_version_freshness" {
  name          = "Version freshness"
  description   = "Which client versions arrive at problem-code docs (from the version param; scoped to arrivals that carry one, i.e. client links). Older versions clustering on a code hint the issue may already be fixed upstream."
  dashboard_ids = [posthog_dashboard.problem_code_reach.id]
  tags          = local.ph_problem_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind = "TrendsQuery"
      series = [{
        kind  = "EventsNode"
        event = "$pageview"
        name  = "$pageview"
        math  = "total"
        properties = [
          { key = "$pathname", type = "event", operator = "icontains", value = ["/problems/"] },
          { key = "version", type = "event", operator = "is_set", value = ["is_set"] },
        ]
      }]
      interval        = "week"
      dateRange       = { date_from = local.ph_date_from }
      breakdownFilter = { breakdowns = [{ property = "version", type = "event" }] }
      trendsFilter    = { display = "ActionsTable" }
    }
  })
}

resource "posthog_insight" "problem_code_referrers" {
  name          = "Referrers to problem pages"
  description   = "Referring domains for problem-code page views. Complements the channel breakdown: shows external pages (search, forums) that send people to a diagnostic's docs — arrivals that carry no channel."
  dashboard_ids = [posthog_dashboard.problem_code_reach.id]
  tags          = local.ph_problem_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind = "TrendsQuery"
      series = [{
        kind       = "EventsNode"
        event      = "$pageview"
        name       = "$pageview"
        math       = "dau"
        properties = local.problem_code_path
      }]
      interval        = "week"
      dateRange       = { date_from = local.ph_date_from }
      breakdownFilter = { breakdowns = [{ property = "$referring_domain", type = "event" }] }
      trendsFilter    = { display = "ActionsTable" }
    }
  })
}
