---
name: Corpus-Sourced Defect
about: Report a defect found by running third-party IEC 61131-3 code or a differential sweep
title: ''
labels: ''
assignees: ''
---

<!--
  READ FIRST: specs/steering/external-corpus-defect-sourcing.md

  This template has NO code section, deliberately. A defect found by running
  third-party code must cross into this repository as prose only, so that the
  fix and its regression test cannot inherit copyright from whatever revealed
  the defect.

  Do NOT include: program text of any length, upstream file paths, upstream
  test or function-block names, upstream comments, or the ordering of upstream
  cases. Describe the construct in words instead.

  Scalar values ARE allowed in prose — a number is a fact about our compiler,
  not somebody's authorship.
-->

**Construct involved**
Name the language construct in words — e.g. "the AND_THEN operator", "an array
of structures indexed by a variable", "STRING passed as VAR_IN_OUT".

**What IronPLC does**
Describe the wrong behaviour in sentences, including any input and output
values.

**What it should do**
Describe the correct behaviour, with the same values.

**Why that is correct**
Cite the IEC 61131-3 clause or table, the documented vendor behaviour, or the
arithmetic. Do not justify it only by "another implementation does it".

**Problem code (if any)**
e.g. P0001, E0001, V4001 — or the VM trap raised. State explicitly if none was
raised when one should have been.

**Dialect and flags**
The `--dialect` value and any `--allow-*` flags in effect.

**Provenance**
The sweep run identifier. Not a file path, and not a corpus file name.

**Notes**
Any other context, in prose.
