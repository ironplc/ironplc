# External Corpus Defect Sourcing Policy

This steering file governs how a defect **discovered by running third-party IEC
61131-3 code** is allowed to become a change in this repository, so that the
change does not carry copyright from the code that revealed it.

> **Not legal advice.** This is an engineering risk-management policy. Questions
> about a specific corpus, or about relaxing any boundary below, require review
> by the project owner and, where warranted, legal counsel.

## Applies To

Anyone — human or AI — who:

- runs third-party IEC 61131-3 source through IronPLC to look for defects, or
- files an issue about a defect found that way, or
- writes a fix or a regression test for such an issue.

The companion catalog of known corpora is
[specs/external-test-corpora.md](../external-test-corpora.md).

For authoring bundled compatibility libraries, see
[Compatibility Library Authoring](compatibility-library-authoring.md). The two
policies address different risks and neither substitutes for the other.

## Why This Exists

Our tests are written by the people who wrote the code under test, so they
inherit its blind spots. Third-party test suites — written by other people, for
other runtimes, carrying their own expected results — are the best independent
oracle available for compiler and runtime defects.

Reading and running that code is unrestricted. Copyright reaches copying,
distribution and derivative works; GPLv3 §2 states that running is not
restricted, and we distribute nothing. The exposure is narrower and more
specific: **a regression test that reproduces an upstream test's case
selection, ordering, identifiers or comments is a derivative of it**, and
shipping that under IronPLC's MIT license would be a violation.

A rule of conduct ("don't copy") cannot be checked after the fact. So this
policy removes the possibility instead, by ensuring the only channel from the
corpus to this repository is one that cannot carry program text.

## The Three Boundaries

### 1. Automation lives in a separate repository

Sweep tooling, fetch recipes, adapters, and the corpora themselves never appear
in `ironplc/ironplc` — not as a submodule, a dependency, a CI step, a
`justfile` recipe, or a gitignored working directory.

This is the boundary that does the real work. "No external corpus is in this
repository" is a claim a reviewer can verify by looking; "nobody copied
anything" is not.

The sweep repository must also protect itself. Either generate every adapter
and wrapper at run time and commit none, or license that repository
GPL-3.0-or-later so that a derivative which does get committed is compliant
rather than infringing. Doing both is free.

### 2. The airlock is a GitHub issue, written in prose, containing no code

A defect crosses into this repository as an independently filed issue that
describes the wrong behaviour in sentences.

Prose is the filter. A sentence can carry a *fact* about IronPLC's behaviour.
It cannot carry upstream's expression, because that expression was program text
and the issue has none.

### 3. The fix is authored from the issue alone

Whoever writes the change and its regression test works from the issue text,
without the corpus open.

The regression test that lands here is **not a port of an upstream test**. It is
a new test, written against IEC 61131-3 and this project's conventions, that
happens to cover behaviour an external corpus pointed at. It will often be
shorter, differently named, and differently organised than whatever found the
defect. That is the intended outcome, not a loss of fidelity.

## Allowed Content in an Issue

- What IronPLC did, described in prose.
- What it should have done, with the justification — a clause of IEC 61131-3, a
  documented vendor behaviour, or plain arithmetic.
- **Scalar values**, stated in sentences. A number is a fact: *"passing 4.0
  returns 1.9999998 where 2.0 is correct"* is an observation about our
  compiler, not somebody's authorship.
- The language construct involved, named — `AND_THEN`, `ARRAY OF STRUCT`,
  `REF_TO`, a standard function's name.
- The diagnostic code or VM trap emitted, or the fact that none was.
- The dialect and `--allow-*` flags in effect.
- A provenance line naming the **sweep run identifier** — not the upstream file.

## Forbidden Content in an Issue

- **Any code block or inline program text**, however short, and regardless of
  whether it was copied or retyped from memory. Describe it instead.
- Upstream file paths, test names, or function-block names.
- Upstream comment prose, translated or paraphrased.
- The ordering or grouping of upstream cases — *"the seven cases it checks, in
  order, are…"* reproduces the selection, which is the protectable part.
- Attachments, screenshots, or diffs containing upstream source.

The forbidden list applies to the issue, its comments, the branch name, commit
messages, test names, and code comments in the fix. The airlock is only as good
as its narrowest point.

## Required Workflow

1. **Sweep** in the separate repository. Its output is a list of failures: our
   invocation, what IronPLC produced, what the correct answer is.
2. **File a prose issue** in `ironplc/ironplc` for each distinct defect, within
   the content rules above, using the corpus-sourced defect template.
3. **Fix and test from the issue.** Author the change and its regression test
   in the existing harnesses, working from the issue text only.
4. **Review** as normal, plus the checklist below.

Where the sweep is *differential* — feeding our own ST programs to two
implementations and comparing — no upstream source is read at all and only
step 1 differs. The remaining steps are unchanged, because a uniform rule is
cheaper to review than a per-corpus judgement.

## Enforcement: automated vs review

- **Automated.** A corpus-sourced defect issue template with no code-fence
  section and a required provenance-line field. A CI assertion that no
  workflow, manifest, submodule, script or `justfile` recipe in this repository
  references the sweep repository or any catalogued corpus. These check that
  the *channel* is intact.
- **Review-only.** That the issue's author actually refrained from
  reconstructing upstream expression in prose, and that the fix's author worked
  from the issue rather than the source. As with the compatibility-library
  policy: the automation checks the shape, the human checks the truth.

## Reviewer Checklist

- The issue contains no program text, upstream paths, or upstream test names.
- The issue's expected behaviour is justified from the standard or from
  arithmetic, not asserted because another implementation does it.
- The regression test reads as ours: house naming
  (`function_when_condition_then_result`), house harness, house structure.
- Nothing in the branch — fixtures, workflows, manifests, comments, commit
  messages — references an external corpus or the sweep repository.
- The provenance line names a run identifier, not a file.
