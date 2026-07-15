# ---------------------------------------------------------------------------
# PostHog "Adoption & Success" dashboard.
#
# One posthog_dashboard + one posthog_insight per tile. Each insight's
# query_json is the raw PostHog query node (an InsightVizNode wrapping a
# TrendsQuery / FunnelsQuery / RetentionQuery), serialized with jsonencode().
#
# The tiles below use only events that are ALREADY flowing into PostHog:
#   - $pageview            (website + playground)
#   - playground_loaded, compile_attempted, compile_finished,
#     run_started, run_stopped, example_loaded  (playground)
#   with super-properties: dialect, host_page, example_name, program_origin.
#
# compile_finished carries error_codes (the diagnostic codes of a failed
# compile) but never the program source, so error-code tiles reveal WHY
# compiles fail without exposing anyone's code.
#
# The one exception is todo_report_submitted: fired only when a user clicks
# "Submit Code" on a P9999 diagnostic, it DOES carry the program source (in the
# `program` property) because fixing P9999 requires seeing the program. That
# transmission is explicit and consented — the button and its consent line tell
# the user their code is shared and may become public — never automatic.
#
# Install-adoption tiles (install_completed / release_downloads / Open VSX)
# depend on collectors not built yet and are left as commented stubs at the
# bottom.
#
# NOTE: query_json field values (boolean property filters, breakdownFilter
# shape, display enums) can vary by PostHog version. Validate with
# `terraform plan` / `terraform apply` and adjust anything the API rejects.
# ---------------------------------------------------------------------------

locals {
  # Shared 90-day trailing window for every insight.
  ph_date_from = "-90d"
  ph_tags      = ["managed-by-terraform", "adoption-success"]
}

resource "posthog_dashboard" "adoption" {
  name        = "IronPLC — Adoption & Success"
  description = "Acquisition → interest → activation → retention, plus product-health tiles. Managed by Terraform (infrastructure/posthog.tf)."
  pinned      = true
  tags        = local.ph_tags
}

# ---------------------------------------------------------------------------
# Section A — Acquisition
# ---------------------------------------------------------------------------

resource "posthog_insight" "weekly_visitors" {
  name          = "Weekly visitors"
  description   = "Unique visitors per week across the website and playground."
  dashboard_ids = [posthog_dashboard.adoption.id]
  tags          = local.ph_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind         = "TrendsQuery"
      series       = [{ kind = "EventsNode", event = "$pageview", name = "$pageview", math = "dau" }]
      interval     = "week"
      dateRange    = { date_from = local.ph_date_from }
      trendsFilter = { display = "ActionsLineGraph" }
    }
  })
}

resource "posthog_insight" "top_pages" {
  name          = "Top pages"
  description   = "Most-viewed paths."
  dashboard_ids = [posthog_dashboard.adoption.id]
  tags          = local.ph_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind            = "TrendsQuery"
      series          = [{ kind = "EventsNode", event = "$pageview", name = "$pageview", math = "total" }]
      interval        = "week"
      dateRange       = { date_from = local.ph_date_from }
      breakdownFilter = { breakdowns = [{ property = "$pathname", type = "event" }] }
      trendsFilter    = { display = "ActionsTable" }
    }
  })
}

resource "posthog_insight" "traffic_sources" {
  name          = "Traffic sources"
  description   = "Unique visitors by referring domain."
  dashboard_ids = [posthog_dashboard.adoption.id]
  tags          = local.ph_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind            = "TrendsQuery"
      series          = [{ kind = "EventsNode", event = "$pageview", name = "$pageview", math = "dau" }]
      interval        = "week"
      dateRange       = { date_from = local.ph_date_from }
      breakdownFilter = { breakdowns = [{ property = "$referring_domain", type = "event" }] }
      trendsFilter    = { display = "ActionsTable" }
    }
  })
}

# ---------------------------------------------------------------------------
# Section B — Interest
# ---------------------------------------------------------------------------

resource "posthog_insight" "install_page_reach" {
  name          = "Install-page reach"
  description   = "Unique visitors who view the installation instructions."
  dashboard_ids = [posthog_dashboard.adoption.id]
  tags          = local.ph_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind = "TrendsQuery"
      series = [{
        kind  = "EventsNode"
        event = "$pageview"
        name  = "$pageview"
        math  = "dau"
        properties = [{
          key      = "$pathname"
          type     = "event"
          operator = "icontains"
          value    = ["/quickstart/installation"]
        }]
      }]
      interval     = "week"
      dateRange    = { date_from = local.ph_date_from }
      trendsFilter = { display = "BoldNumber" }
    }
  })
}

resource "posthog_insight" "playground_reach" {
  name          = "Playground reach"
  description   = "Unique users who load the playground."
  dashboard_ids = [posthog_dashboard.adoption.id]
  tags          = local.ph_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind         = "TrendsQuery"
      series       = [{ kind = "EventsNode", event = "playground_loaded", name = "playground_loaded", math = "dau" }]
      interval     = "week"
      dateRange    = { date_from = local.ph_date_from }
      trendsFilter = { display = "ActionsLineGraph" }
    }
  })
}

# ---------------------------------------------------------------------------
# Section C — Activation ("success")
# ---------------------------------------------------------------------------

resource "posthog_insight" "successful_compiles" {
  name          = "Successful compiles"
  description   = "Unique users with a successful playground compile per week. North-star proxy."
  dashboard_ids = [posthog_dashboard.adoption.id]
  tags          = local.ph_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind = "TrendsQuery"
      series = [{
        kind       = "EventsNode"
        event      = "compile_finished"
        name       = "compile_finished"
        math       = "dau"
        properties = [{ key = "success", type = "event", operator = "exact", value = [true] }]
      }]
      interval     = "week"
      dateRange    = { date_from = local.ph_date_from }
      trendsFilter = { display = "ActionsLineGraph" }
    }
  })
}

resource "posthog_insight" "compile_success_rate" {
  name          = "Compile success rate"
  description   = "Successful compiles / all compiles (formula A/B)."
  dashboard_ids = [posthog_dashboard.adoption.id]
  tags          = local.ph_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind = "TrendsQuery"
      series = [
        {
          kind       = "EventsNode"
          event      = "compile_finished"
          name       = "compile_finished (success)"
          math       = "total"
          properties = [{ key = "success", type = "event", operator = "exact", value = [true] }]
        },
        {
          kind  = "EventsNode"
          event = "compile_finished"
          name  = "compile_finished (all)"
          math  = "total"
        },
      ]
      interval     = "week"
      dateRange    = { date_from = local.ph_date_from }
      trendsFilter = { display = "ActionsLineGraph", formula = "A/B" }
    }
  })
}

resource "posthog_insight" "programs_run" {
  name          = "Programs run"
  description   = "Unique users who start a run per week."
  dashboard_ids = [posthog_dashboard.adoption.id]
  tags          = local.ph_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind         = "TrendsQuery"
      series       = [{ kind = "EventsNode", event = "run_started", name = "run_started", math = "dau" }]
      interval     = "week"
      dateRange    = { date_from = local.ph_date_from }
      trendsFilter = { display = "ActionsLineGraph" }
    }
  })
}

# ---------------------------------------------------------------------------
# Section D — Health / friction
# ---------------------------------------------------------------------------

resource "posthog_insight" "broken_docs_examples" {
  name          = "Broken docs examples"
  description   = "Failed compiles of as-shipped docs examples, broken down by the docs page hosting the embed. Scoped to program_origin=docs (only playgrounds embedded in docs pages) and program_modified=false (the example as shipped, not a visitor's edits), so every row is a genuinely broken example rather than general playground experimentation."
  dashboard_ids = [posthog_dashboard.adoption.id]
  tags          = local.ph_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind = "TrendsQuery"
      series = [{
        kind  = "EventsNode"
        event = "compile_finished"
        name  = "compile_finished"
        math  = "total"
        properties = [
          { key = "success", type = "event", operator = "exact", value = [false] },
          { key = "program_origin", type = "event", operator = "exact", value = ["docs"] },
          { key = "program_modified", type = "event", operator = "exact", value = [false] },
        ]
      }]
      interval        = "week"
      dateRange       = { date_from = local.ph_date_from }
      breakdownFilter = { breakdowns = [{ property = "host_page", type = "event" }] }
      trendsFilter    = { display = "ActionsTable" }
    }
  })
}

resource "posthog_insight" "top_compile_error_codes" {
  name          = "Top compile error codes"
  description   = "Diagnostic codes (e.g. P####) from failed playground compiles, ranked by frequency. Reveals why compiles fail without capturing any program source — only the error code is collected."
  dashboard_ids = [posthog_dashboard.adoption.id]
  tags          = local.ph_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind = "TrendsQuery"
      series = [{
        kind       = "EventsNode"
        event      = "compile_finished"
        name       = "compile_finished"
        math       = "total"
        properties = [{ key = "success", type = "event", operator = "exact", value = [false] }]
      }]
      interval        = "week"
      dateRange       = { date_from = local.ph_date_from }
      breakdownFilter = { breakdowns = [{ property = "error_codes", type = "event" }] }
      trendsFilter    = { display = "ActionsBarValue" }
    }
  })
}

resource "posthog_insight" "todo_report_submissions" {
  name          = "P9999 code submissions"
  description   = "Programs users chose to submit (via the playground \"Submit Code\" button) after hitting P9999 — the capability-not-implemented error. Each event carries the program source so the reported feature can be added. Unlike the error-code tiles, this event intentionally includes source, transmitted only on explicit, consented user action."
  dashboard_ids = [posthog_dashboard.adoption.id]
  tags          = local.ph_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind = "TrendsQuery"
      series = [{
        kind  = "EventsNode"
        event = "todo_report_submitted"
        name  = "todo_report_submitted"
        math  = "total"
      }]
      interval     = "week"
      dateRange    = { date_from = local.ph_date_from }
      trendsFilter = { display = "ActionsLineGraph" }
    }
  })
}

resource "posthog_insight" "top_compiler_error_locations" {
  name          = "Top P9 compiler error locations"
  description   = "Compiler source file#line of P9xxx errors (unimplemented capabilities / internal errors) from failed playground compiles, ranked by frequency. This is the compiler's own location — collected automatically because it never contains any program source — and it points maintainers straight at the code that needs work."
  dashboard_ids = [posthog_dashboard.adoption.id]
  tags          = local.ph_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind = "TrendsQuery"
      series = [{
        kind       = "EventsNode"
        event      = "compile_finished"
        name       = "compile_finished"
        math       = "total"
        properties = [{ key = "success", type = "event", operator = "exact", value = [false] }]
      }]
      interval        = "week"
      dateRange       = { date_from = local.ph_date_from }
      breakdownFilter = { breakdowns = [{ property = "error_locations", type = "event" }] }
      trendsFilter    = { display = "ActionsBarValue" }
    }
  })
}

resource "posthog_insight" "top_error_codes" {
  name          = "Top runtime error codes"
  description   = "Error codes from runs that stopped on an error while executing (as opposed to failing to compile)."
  dashboard_ids = [posthog_dashboard.adoption.id]
  tags          = local.ph_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind = "TrendsQuery"
      series = [{
        kind       = "EventsNode"
        event      = "run_stopped"
        name       = "run_stopped"
        math       = "total"
        properties = [{ key = "reason", type = "event", operator = "exact", value = ["error"] }]
      }]
      interval        = "week"
      dateRange       = { date_from = local.ph_date_from }
      breakdownFilter = { breakdowns = [{ property = "error_codes", type = "event" }] }
      trendsFilter    = { display = "ActionsBarValue" }
    }
  })
}

resource "posthog_insight" "dialect_adoption" {
  name          = "Dialect adoption"
  description   = "Playground loads by IEC 61131-3 dialect (2003 vs 2013)."
  dashboard_ids = [posthog_dashboard.adoption.id]
  tags          = local.ph_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind            = "TrendsQuery"
      series          = [{ kind = "EventsNode", event = "playground_loaded", name = "playground_loaded", math = "total" }]
      interval        = "week"
      dateRange       = { date_from = local.ph_date_from }
      breakdownFilter = { breakdowns = [{ property = "dialect", type = "event" }] }
      trendsFilter    = { display = "ActionsPie" }
    }
  })
}

resource "posthog_insight" "example_popularity" {
  name          = "Example popularity"
  description   = "Which built-in examples users load."
  dashboard_ids = [posthog_dashboard.adoption.id]
  tags          = local.ph_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind            = "TrendsQuery"
      series          = [{ kind = "EventsNode", event = "example_loaded", name = "example_loaded", math = "total" }]
      interval        = "week"
      dateRange       = { date_from = local.ph_date_from }
      breakdownFilter = { breakdowns = [{ property = "example_name", type = "event" }] }
      trendsFilter    = { display = "ActionsTable" }
    }
  })
}

# ---------------------------------------------------------------------------
# Section E — Visitor → success funnel
# ---------------------------------------------------------------------------

resource "posthog_insight" "visitor_success_funnel" {
  name          = "Visitor → success funnel"
  description   = "Pageview → playground load → successful compile → run, over a 7-day window. Steps 1→2 cross the www/playground subdomain boundary (directional for iframe embeds)."
  dashboard_ids = [posthog_dashboard.adoption.id]
  tags          = local.ph_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind = "FunnelsQuery"
      series = [
        { kind = "EventsNode", event = "$pageview", name = "$pageview" },
        { kind = "EventsNode", event = "playground_loaded", name = "playground_loaded" },
        {
          kind       = "EventsNode"
          event      = "compile_finished"
          name       = "compile_finished (success)"
          properties = [{ key = "success", type = "event", operator = "exact", value = [true] }]
        },
        { kind = "EventsNode", event = "run_started", name = "run_started" },
      ]
      dateRange     = { date_from = local.ph_date_from }
      funnelsFilter = { funnelWindowInterval = 7, funnelWindowIntervalUnit = "day" }
    }
  })
}

# ---------------------------------------------------------------------------
# Section F — Retention
# ---------------------------------------------------------------------------

resource "posthog_insight" "compile_retention" {
  name          = "Compile retention"
  description   = "Of users who compile in a week, how many return to compile in later weeks."
  dashboard_ids = [posthog_dashboard.adoption.id]
  tags          = local.ph_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind = "RetentionQuery"
      retentionFilter = {
        period          = "Week"
        totalIntervals  = 8
        retentionType   = "retention_first_time"
        targetEntity    = { id = "compile_finished", name = "compile_finished", type = "events" }
        returningEntity = { id = "compile_finished", name = "compile_finished", type = "events" }
      }
      dateRange = { date_from = local.ph_date_from }
    }
  })
}

# ---------------------------------------------------------------------------
# Section G — Engagement (lifecycle & stickiness)
#
# Both run on compile_finished (the activation event) over the shared 90-day
# window, so they need no new instrumentation — they re-cut events already
# flowing into PostHog.
# ---------------------------------------------------------------------------

resource "posthog_insight" "compile_lifecycle" {
  name          = "Compile lifecycle"
  description   = "New, returning, resurrecting, and dormant users per week, measured on compile_finished. Complements the retention tile: retention asks 'do they come back', lifecycle asks 'what is the weekly mix of new vs. returning vs. lapsed compilers'."
  dashboard_ids = [posthog_dashboard.adoption.id]
  tags          = local.ph_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind            = "LifecycleQuery"
      series          = [{ kind = "EventsNode", event = "compile_finished", name = "compile_finished", math = "total" }]
      interval        = "week"
      dateRange       = { date_from = local.ph_date_from }
      lifecycleFilter = { showLegend = true }
    }
  })
}

resource "posthog_insight" "compile_stickiness" {
  name          = "Compile stickiness"
  description   = "Of users who compile in a given week, on how many distinct days do they compile? A right-shifted distribution means users return within the week, not just once — an engagement signal that retention (week-over-week) does not capture."
  dashboard_ids = [posthog_dashboard.adoption.id]
  tags          = local.ph_tags

  query_json = jsonencode({
    kind = "InsightVizNode"
    source = {
      kind      = "StickinessQuery"
      series    = [{ kind = "EventsNode", event = "compile_finished", name = "compile_finished", math = "total" }]
      interval  = "day"
      dateRange = { date_from = local.ph_date_from }
    }
  })
}

# ---------------------------------------------------------------------------
# STUBS — install-adoption tiles. Uncomment once the collectors emit these
# events (see the Tier 1 / Tier 2 follow-ups). They need no schema guesswork
# beyond the event names below.
# ---------------------------------------------------------------------------
#
# resource "posthog_insight" "cli_installs" {
#   name          = "CLI installs by platform"
#   description   = "install.sh completions, broken down by os/arch (Tier 2)."
#   dashboard_ids = [posthog_dashboard.adoption.id]
#   tags          = local.ph_tags
#   query_json = jsonencode({
#     kind = "InsightVizNode"
#     source = {
#       kind            = "TrendsQuery"
#       series          = [{ kind = "EventsNode", event = "install_completed", name = "install_completed", math = "total" }]
#       interval        = "week"
#       dateRange       = { date_from = local.ph_date_from }
#       breakdownFilter = { breakdowns = [{ property = "os", type = "event" }] }
#       trendsFilter    = { display = "ActionsBarValue" }
#     }
#   })
# }
#
# resource "posthog_insight" "release_downloads" {
#   name          = "Release downloads by platform"
#   description   = "GitHub release asset downloads per platform (Tier 1 collector)."
#   dashboard_ids = [posthog_dashboard.adoption.id]
#   tags          = local.ph_tags
#   query_json = jsonencode({
#     kind = "InsightVizNode"
#     source = {
#       kind            = "TrendsQuery"
#       series          = [{ kind = "EventsNode", event = "release_downloads", name = "release_downloads", math = "sum", math_property = "count" }]
#       interval        = "week"
#       dateRange       = { date_from = local.ph_date_from }
#       breakdownFilter = { breakdowns = [{ property = "platform", type = "event" }] }
#       trendsFilter    = { display = "ActionsBarValue" }
#     }
#   })
# }
