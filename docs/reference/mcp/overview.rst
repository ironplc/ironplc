========
Overview
========

The IronPLC MCP server is a thin adapter that exposes the IronPLC compiler
to any `Model Context Protocol <https://modelcontextprotocol.io/>`_
client — typically an AI coding agent such as Claude Desktop, Cline, or
Claude Code. Each MCP tool corresponds to a capability of the compiler
(syntax check, semantic analysis, symbol extraction, code generation) and
returns structured JSON that the agent can act on without parsing
human-oriented terminal output.

Transport
=========

:program:`ironplcmcp` communicates over **stdio JSON-RPC**. The MCP client
launches :program:`ironplcmcp` as a subprocess, writes requests to its
standard input, and reads responses from its standard output. The server
does not open a network port and does not listen on any socket. Log
output is written to standard error so that it cannot corrupt the
JSON-RPC stream.

The server exits when its client disconnects.

Stateless Design
================

Every tool call supplies its own ``sources`` and ``options`` — the server
keeps no per-session project state between calls. This means the same
call always produces the same result, and an agent can run multiple
conversations against one server without cross-talk.

The one exception is the **container cache**. A successful :literal:`compile`
call stores the compiled bytecode in an in-process LRU cache and returns
a :literal:`container_id`. Later execution tools can refer back to that
identifier instead of re-sending the full source. The cache evicts on
LRU pressure automatically; use :literal:`container_drop` to release an
entry explicitly.

Relationship to the Compiler
============================

The MCP server runs the same parse, analysis, and codegen pipeline as
:doc:`ironplcc </reference/compiler/ironplcc>`. Diagnostics use the same
problem codes (see :doc:`/reference/compiler/problems/index`) and dialect
and feature-flag names match the CLI's :literal:`--dialect` and
:literal:`--allow-*` flags. Given identical inputs, an MCP tool and the
corresponding CLI invocation produce identical results.

When to Use Which Tool
======================

* Use :literal:`parse` while drafting to confirm the source tokenizes.
* Use :literal:`check` before declaring a change correct — it catches
  type errors, undeclared symbols, and the rest of the semantic rules.
* Use :literal:`compile` only when you need a bytecode artifact to run.
  For validation, :literal:`check` is faster and produces the same
  diagnostics.

See :doc:`tools` for the full catalog of tools and their inputs and
outputs.

See Also
========

* :doc:`ironplcmcp` --- Command-line reference for the server binary
* :doc:`tools` --- Reference entry for each MCP tool
* :doc:`/how-to-guides/ai-agents/write-plc-programs-with-an-ai-agent`
  --- End-to-end guide for configuring and using an AI agent with IronPLC
* :doc:`/reference/compiler/index` --- Compiler CLI reference
