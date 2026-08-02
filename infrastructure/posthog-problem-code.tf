# ---------------------------------------------------------------------------
# PostHog "Problem-code reach" dashboard.
#
# Problem-code documentation pages (…/reference/<section>/problems/<CODE>.html)
# are linked from every IronPLC surface: the playground, the CLI, the editor
# extension, and the MCP server. Each link carries UTM metadata added by the
# client:
#   - utm_source   = playground | cli | extension | mcp   (the channel)
#   - utm_medium   = problem-code                          (constant)
#   - utm_campaign = <client version>                      (mirrors ?version)
#
# The docs site emits $pageview on every page (docs/_static/posthog-init.js),
# and PostHog auto-extracts the utm_* params into event properties. These tiles
# re-cut that existing $pageview stream — no new instrumentation — to answer
# "from where, and on which version, do people reach our problem-code docs?".
#
# Every tile is scoped to problem-code pages reached via a diagnostic link
# (local.problem_code_props): $pathname contains "/problems/" AND
# utm_medium = "problem-code". Scoping on utm_medium (not just the path) keeps
# organic docs navigation to the same pages out of the channel breakdowns.
#
# Cardinality note: the client also appends ?version, and the LSP/CLI paths add
# &file/&line. Those never fragment these tiles because every insight breaks
# down on a low-cardinality property ($pathname, utm_source, utm_campaign,
# $referring_domain) rather than the full URL.
#
# NOTE: query_json field values can vary by PostHog version. Validate with
# `terraform plan` / `terraform apply` and adjust anything the API rejects.
# ---------------------------------------------------------------------------

locals {
  ph_problem_tags = ["managed-by-terraform", "problem-code-reach"]

  # Shared scope: problem-code doc pages arrived at via a tagged diagnostic
  # link. Applied to every series below.
  problem_code_props = [
    { key = "$pathname", type = "event", operator = "icontains", value = ["/problems/"] },
    { key = "utm_medium", type = "event", operator = "exact", value = ["problem-code"] },
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
  description   = "Total problem-code page views reached via a diagnostic link, over the trailing window."
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
        properties = local.problem_code_props
      }]
      interval     = "week"
      dateRange    = { date_from = local.ph_date_from }
      trendsFilter = { display = "BoldNumber" }
    }
  })
}

resource "posthog_insight" "problem_code_reach_by_channel" {
  name          = "Reach by channel"
  description   = "Unique visitors reaching problem-code docs, broken down by utm_source (playground / cli / extension / mcp). The headline adoption-by-channel tile."
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
        properties = local.problem_code_props
      }]
      interval        = "week"
      dateRange       = { date_from = local.ph_date_from }
      breakdownFilter = { breakdowns = [{ property = "utm_source", type = "event" }] }
      trendsFilter    = { display = "ActionsTable" }
    }
  })
}

resource "posthog_insight" "problem_code_channel_trend" {
  name          = "Channel trend"
  description   = "Weekly unique visitors reaching problem-code docs per channel (utm_source), over time."
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
        properties = local.problem_code_props
      }]
      interval        = "week"
      dateRange       = { date_from = local.ph_date_from }
      breakdownFilter = { breakdowns = [{ property = "utm_source", type = "event" }] }
      trendsFilter    = { display = "ActionsLineGraph" }
    }
  })
}

# ---------------------------------------------------------------------------
# What and who
# ---------------------------------------------------------------------------

resource "posthog_insight" "problem_code_top_codes" {
  name          = "Top problem codes"
  description   = "Most-reached problem-code pages, by path. Reveals which diagnostics people actually follow the docs link for."
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
        properties = local.problem_code_props
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
  description   = "Which client versions arrive at problem-code docs, from utm_campaign (mirrors the client version). Older versions clustering on a code hint the issue may already be fixed upstream."
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
        properties = local.problem_code_props
      }]
      interval        = "week"
      dateRange       = { date_from = local.ph_date_from }
      breakdownFilter = { breakdowns = [{ property = "utm_campaign", type = "event" }] }
      trendsFilter    = { display = "ActionsTable" }
    }
  })
}

resource "posthog_insight" "problem_code_referrers" {
  name          = "Referrers to problem pages"
  description   = "Referring domains for problem-code page views. Complements utm_source: shows external pages (search, forums) that send people to a diagnostic's docs."
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
        properties = local.problem_code_props
      }]
      interval        = "week"
      dateRange       = { date_from = local.ph_date_from }
      breakdownFilter = { breakdowns = [{ property = "$referring_domain", type = "event" }] }
      trendsFilter    = { display = "ActionsTable" }
    }
  })
}
