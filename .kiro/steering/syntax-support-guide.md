---
inclusion: fileMatch
fileMatchPattern: "**/parser/**,**/analyzer/**,**/codegen/**,**/plc2plc/**"
---

# Syntax Support Guide

See [specs/steering/syntax-support-guide.md](../../specs/steering/syntax-support-guide.md) for the full syntax support guidance.

This file describes the complete checklist and patterns for adding new syntax support to the compiler, including lexer tokens, parser rules, non-standard gating with `--allow-x` flags, plc2plc round-trip tests, and end-to-end execution tests.
