# Plan: Add PostHog pageview analytics to the website

## Goal

Add PostHog to the main marketing/documentation website (`docs/`, the Sphinx +
furo site published to https://www.ironplc.com/) so that ordinary visitor
traffic generates a steady stream of pageview events. The website receives
regular visitors (unlike the playground, which is published on the same weekly
cadence but sees far less traffic), so this gives a continuous signal that the
PostHog ingestion pipeline works end-to-end.

## Scope

In scope:
- New `docs/_static/posthog-init.js` — the official PostHog browser loader
  snippet plus an `init` call, configured for pageview capture.
- Register the loader in `docs/conf.py` via `html_js_files`.

Out of scope:
- The playground integration (already present in `playground/posthog-init.js`).
- Removing or changing the existing Clicky tracker (kept as-is).
- A reverse proxy for PostHog (noted as a future improvement; ad/tracker
  blockers will drop some events without it).
- Any cookie-consent banner. The site already ships Clicky with no consent
  banner, so adding PostHog follows the established privacy posture.

## Decisions

- **Project:** reuse the same PostHog project key/host as the playground
  (`phc_xQV6wYsbu5FuF5AuCkwXgZqcdtcURBi5BoLrPu9Mar7L`,
  `https://us.i.posthog.com`). Website and playground data share one project.
- **Capture scope:** pageviews only. `capture_pageview: true`,
  `autocapture: false`, `disable_session_recording: true`,
  `person_profiles: 'identified_only'`. This yields visitors / pageviews /
  referrers without click-level autocapture or recordings.

## Architecture

The furo theme already injects custom static JavaScript through Sphinx's
`html_js_files` (e.g. `version-check.js`). PostHog's loader is shipped as a
static file and registered the same way; Sphinx emits a
`<script src="_static/posthog-init.js">` tag in every page's `<head>`. This is
the clean alternative to the existing Clicky hack (which is embedded inside a
`footer_icons` HTML blob because it loads a third-party CDN URL rather than a
local file).

The loader uses PostHog's official async snippet: it defines a stubbed
`window.posthog` that queues calls, asynchronously loads the real SDK
(`array.js`), then replays the queue. `posthog.init(...)` with
`capture_pageview: true` records a `$pageview` on load.

## Deployment

The website is published by the `publish-website` job in `deployment.yaml`,
which runs on the weekly Monday release (or a manual `workflow_dispatch` with
`dryrun` unchecked). The change therefore goes live with the next release, not
on merge to `main`.

## Testing

- `cd docs && just ci` must pass (Sphinx build with `-W -n`, warnings as
  errors). Adding a static asset and an `html_js_files` entry must not
  introduce build warnings.
- Manual verification after deploy: load https://www.ironplc.com/, confirm a
  request to `https://us.i.posthog.com/...` fires and a `$pageview` appears in
  PostHog.
