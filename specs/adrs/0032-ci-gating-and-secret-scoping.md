# CI Gating for Untrusted PRs and Secret Scoping via Environments

status: proposed
date: 2026-04-27

## Context and Problem Statement

GitHub Actions workflows in this repository span three trust contexts:

1. **PR validation** (`integration.yaml`) — runs untrusted code from forks or unreviewed branches on maintainer-paid runners.
2. **Scheduled dependency updates** (`update.yaml`) — runs trusted code on a Sunday cron, holds a token that bypasses branch protection on `main`.
3. **Scheduled releases** (`deployment.yaml`) — runs trusted code on a Monday cron, holds tokens that publish to the VS Code Marketplace, Open VSX, GitHub Pages (playground), and the Homebrew tap.

The risks differ:

* A fork PR can run arbitrary code on a runner. With no secrets in scope, the impact is bounded to runner-minute abuse and a read-only `GITHUB_TOKEN`. With publish secrets in scope, the impact is a compromised release artifact reaching every IronPLC user.
* A release-time secret leak is catastrophic. `IRONPLC_WORKFLOW_PUBLISH_ACCESS_TOKEN` can push to `main` (bypassing branch protection); `VS_MARKETPLACE_TOKEN`/`OVSX_PAT` can ship a tampered extension to thousands of users.

The repository previously gated PR validation with per-job `if:` conditions that skipped jobs whose author was not an `OWNER`/`MEMBER`/`COLLABORATOR`. Two problems surfaced (see PR #979):

* Skipped checks do not satisfy required-status-check rules, so external PRs hung in `blocked` state with no UI affordance for maintainers to opt in to a specific PR.
* Publish secrets were stored at the repository level, so any workflow — including a fork PR with a tampered workflow file — could in principle reference them.

## Decision Drivers

* **Untrusted contributors must not consume runner minutes or execute code without explicit, per-commit maintainer approval.**
* **Publish secrets must be unreachable from PR-triggered workflows**, even if the workflow file is tampered with by the PR author.
* **Scheduled builds must run unattended** — the weekly dependency update and weekly release cannot wait on a human approver each cron tick.
* **GitHub's repo-level "Require approval for first-time contributors" setting is per-author, not per-PR.** Once a contributor's first run is approved, every subsequent PR from that account runs automatically — turning a single vetting into a permanent whitelist.
* **The workflow file on `pull_request` events comes from the PR branch.** Any gate expressed only as `if:` or `needs:` inside the YAML can be edited away by the PR author. Repo-level configuration cannot.
* **`IRONPLC_WORKFLOW_PUBLISH_ACCESS_TOKEN` is a fine-grained PAT with `Contents: write` on three repos** (`ironplc/ironplc`, `ironplc/ironplc-playground`, `ironplc/homebrew-brew`) and is in the `main`-branch-protection bypass list. Its blast radius is the entire org.

## Considered Options

* Per-job author-association `if:` gates (the previous approach)
* Repo-level "first-time contributor" approval setting
* Label-gated workflow with a `safe-to-test` label
* Two GitHub Environments — `pr-ci` (per-run approval, no secrets) and `production` (secret scoping, branch-restricted)
* `pull_request_target` for everything

## Decision Outcome

Chosen option: **Two GitHub Environments — `pr-ci` and `production`.**

| Environment | Required reviewers | Deployment branches | Secrets |
|---|---|---|---|
| `pr-ci` | Yes (maintainers) | Any | None |
| `production` | No | `main` only | `IRONPLC_WORKFLOW_PUBLISH_ACCESS_TOKEN`, `VS_MARKETPLACE_TOKEN`, `OVSX_PAT` |

The `pr-ci` environment is referenced by a single zero-step `approve` job at the top of `integration.yaml`. Every other job in that workflow declares `needs: approve`. Every PR run pauses with "Awaiting maintainer approval" until a reviewer clicks Approve, scoped to that exact commit. Pushing a new commit invalidates the approval and queues a new pending one.

The `production` environment holds the publish secrets. It has no required reviewers — the weekly cron runs unattended. Its deployment-branch restriction to `main` means that even a `workflow_dispatch` from a feature branch cannot deploy to it.

Jobs that declare `environment: production`:

* `partial_version.yaml::release` — pushes version commits and tags to `main`
* `partial_update_dependencies.yaml::update-dependencies` — pushes a feature branch
* `update.yaml::merge-changes` — merges the dependency-update branch to `main`
* `deployment.yaml::publish-release` — VS Code Marketplace and Open VSX
* `deployment.yaml::publish-playground` — pushes to `ironplc/ironplc-playground`
* `deployment.yaml::publish-homebrew` — pushes to `ironplc/homebrew-brew`

The single job that declares `environment: pr-ci`:

* `integration.yaml::approve` — zero-step gate; every other job in the workflow declares `needs: approve`.

The two layers compose: `pr-ci` answers "is this code allowed to run at all?", `production` answers "is this code allowed to publish?" — and the two sets of jobs never overlap.

### Least-privilege `GITHUB_TOKEN` scoping

The two environments scope the long-lived publish *secrets*. A third layer scopes the ephemeral `GITHUB_TOKEN`: its `contents` permission is `read` by default, and only the jobs that genuinely mutate the repository or a release opt up to `write`.

Previously `deployment.yaml` declared `contents: write` (plus unused `pages: write` and `id-token: write`) at the workflow level, which flowed into every reusable build workflow that did not declare its own `permissions:`. The build jobs (`partial_compiler.yaml`, `partial_vscode_extension.yaml`) then uploaded assets directly to the GitHub Release with `svenstaro/upload-release-action` — meaning a write-capable token was in scope across the largest attack surface in the pipeline: the jobs that run the most third-party actions and compile arbitrary dependency code (`just ci`, `cargo install`, `choco install`, rust-cache, cargo-llvm-cov).

The build jobs now upload only to build-artifact storage via `actions/upload-artifact` (which needs no `contents: write`). A single consolidated job, `partial_upload_release_artifacts.yaml`, downloads those artifacts and attaches them to the release. The result is that a write-capable `GITHUB_TOKEN` exists in only three credentialed stages:

1. **Create version** — `partial_version.yaml::release` (also holds the PAT).
2. **Upload release artifacts** — `partial_upload_release_artifacts.yaml::upload-release-artifacts`.
3. **Publish** — `publish-website`, `publish-release`, `cleanup` (`GITHUB_TOKEN` write); `publish-playground`, `publish-homebrew` (PAT only, no `GITHUB_TOKEN` write).

`partial_upload_release_artifacts.yaml` uses only `GITHUB_TOKEN`, not a publish secret, so it deliberately does **not** declare `environment: production` (see the GITHUB_TOKEN-only rule in Confirmation item 4). Do not add it.

Reusable-workflow `GITHUB_TOKEN` permissions are the intersection of caller and callee. Callees declare their own minimal `permissions:` as defense-in-depth; a build partial declaring `contents: read` can never write even if a caller misconfigures. `partial_website.yaml` declares the maximum it may need (`contents: write`, for the gh-pages publish push) and relies on callers that only build (`publish: false`) to cap it to `read` via their own read-only scope.

Because the build/test/lint jobs now hold no write token and no secrets, the third-party actions they use can be updated with no credential-theft risk — the motivating goal of this scoping.

`pages: write` and `id-token: write` were removed from `deployment.yaml`: nothing uses the GitHub Pages deploy API or OIDC (`peaceiris/actions-gh-pages` pushes a branch with a token). Re-add either at the specific job if OIDC/trusted-publishing is ever adopted.

### Consequences

* Good, because PR runs cannot reach publish secrets even with a fully tampered workflow file — the secrets are not in repo scope at all.
* Good, because PR runs require explicit per-commit maintainer approval, scoped to that exact SHA.
* Good, because scheduled releases continue to run unattended on the weekly cron.
* Good, because `production` can only be deployed to from `main`, so accidental publishing from a feature branch is prevented by GitHub itself, not by workflow logic.
* Good, because environment configuration lives at the repo level and cannot be modified by editing files in a PR.
* Bad, because removing an `environment:` key is a one-line edit that a future contributor or agent could make in good faith, unaware of the security implication. Mitigated by the YAML comments referencing this ADR and by a CODEOWNERS rule on `.github/workflows/`.
* Bad, because the `pr-ci` gate adds a second-level approval on top of branch protection. Maintainers must click Approve on each PR they push, including their own.
* Neutral, because the workflow file is still read from the PR branch on `pull_request` events. A malicious author can edit `.github/workflows/integration.yaml` to remove `needs: approve`. They still cannot access publish secrets (those live in `production`, which `pr-ci`-context jobs never enter), and they cannot push to the repo. Worst-case impact reduces to "arbitrary code on a runner with a read-only `GITHUB_TOKEN` and no secrets" — the GitHub baseline for any fork PR.

### Confirmation

For any PR that modifies `.github/workflows/**`:

1. Does it remove or weaken `environment: production` on a job that consumes a publish secret?
   * If yes, the PR must explain why and revise this ADR.
2. Does it remove `environment: pr-ci` or `needs: approve` from `integration.yaml`?
   * If yes, the PR must explain how untrusted PRs will instead be gated.
3. Does it add a new publish secret?
   * It must be added to the `production` environment, not to repo-level secrets, and the consuming job must declare `environment: production`.
4. Does it add a new workflow that reads `secrets.*`?
   * The reading job must declare an appropriate environment (`production` for publish secrets; none for `GITHUB_TOKEN`-only).
5. Does a build/test job declare `contents: write`, upload directly to a release (e.g. `svenstaro/upload-release-action`, `softprops/action-gh-release`), or otherwise hold a write-capable token?
   * If yes, it must instead upload to build-artifact storage via `actions/upload-artifact` and let the consolidated `upload-release-artifacts` job attach the assets. Build/test jobs must run with `contents: read` and no secrets so the actions they use can be updated without credential-theft risk. See "Least-privilege `GITHUB_TOKEN` scoping".

A CODEOWNERS rule on `.github/workflows/**` and `**/justfile` ensures a security-aware reviewer is automatically routed to every such PR. Justfiles are included because privileged jobs (`environment: production`) execute justfile recipes with publish secrets in scope — a malicious recipe edit has the same blast radius as a malicious workflow edit.

### One-time GitHub setup

Recorded here so a future maintainer rebuilding the org can recreate the configuration.

1. **Create the `pr-ci` environment** (Settings → Environments → New environment).
   * Required reviewers: enabled, with all maintainers added.
   * Deployment branches and tags: any (must accept fork PR refs).
   * Environment secrets: none.
2. **Create the `production` environment.**
   * Required reviewers: disabled.
   * Deployment branches and tags: selected branches → `main` only.
   * Environment secrets: move `IRONPLC_WORKFLOW_PUBLISH_ACCESS_TOKEN`, `VS_MARKETPLACE_TOKEN`, `OVSX_PAT` from repository secrets.
3. **`IRONPLC_WORKFLOW_PUBLISH_ACCESS_TOKEN` PAT scope** (fine-grained):
   * Resource owner: `ironplc`.
   * Selected repositories: `ironplc/ironplc`, `ironplc/ironplc-playground`, `ironplc/homebrew-brew`.
   * Permissions: `Contents: Read and write`; `Workflows: Read and write` on `ironplc/ironplc`; `Metadata: Read` (auto).
   * The PAT owner must be in the `main`-branch-protection bypass list on `ironplc/ironplc`.
4. **CODEOWNERS** at `.github/CODEOWNERS` should route `.github/workflows/**` to a security-aware reviewer, and `main`-branch protection must enable "Require review from Code Owners".

## Pros and Cons of the Options

### Per-job author-association `if:` gates

* Good, because they're declarative inside the workflow file
* Bad, because they skip rather than queue — required checks are never satisfied for external PRs (PR #979)
* Bad, because they offer no UI affordance for maintainers to opt in to a specific PR
* Bad, because they evaluate against an ever-growing implicit whitelist (`OWNER`/`MEMBER`/`COLLABORATOR`)
* Bad, because the workflow file comes from the PR branch, so the `if:` itself is editable by the author

### Repo-level "first-time contributor" approval

* Good, because it's a single setting toggle and requires no workflow changes
* Bad, because it whitelists per author after the first approval — one innocuous PR earns permanent automatic-run privileges
* Bad, because it doesn't address secret scoping at all

### Label-gated workflow with `safe-to-test`

* Good, because it's per-PR rather than per-author
* Good, because it leaves a label-shaped audit trail
* Bad, because the workflow file is in the PR branch, so the gate itself is editable
* Bad, because it requires a second workflow running from base via `pull_request_target` to strip the label on each push, adding moving parts
* Neutral, because secret scoping has to be solved separately

### Two environments (`pr-ci` + `production`) — chosen

* Good, because environment configuration lives at the repo level and is unaffected by edits in a PR
* Good, because secret scoping is enforced by GitHub itself, not by workflow logic
* Good, because it cleanly separates "allowed to run?" (pr-ci) from "allowed to publish?" (production)
* Good, because branch-restriction on `production` makes accidental publishing from a feature branch impossible
* Bad, because it requires one-time setup outside the codebase (configuring environments in the GitHub UI)
* Bad, because the `approve` gate adds friction to maintainers' own PRs

### `pull_request_target` for everything

* Good, because the workflow file is read from the base branch — PR authors cannot edit it
* Bad, because `pull_request_target` jobs run with full secret access by default — easy to accidentally check out and execute PR code, leaking secrets
* Bad, because it is a known foot-gun (multiple high-profile incidents in the wild)
* Bad, because it makes simple build-and-test workflows harder to write safely
