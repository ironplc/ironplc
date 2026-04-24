==========
ironplcmcp
==========

Name
====

ironplcmcp --- IronPLC Model Context Protocol server

Synopsis
========

| :program:`ironplcmcp`

Description
===========

:program:`ironplcmcp` is the IronPLC Model Context Protocol server. It
exposes the IronPLC compiler as a set of MCP tools over a stdio JSON-RPC
transport, allowing an AI coding agent to parse, validate, and compile
IEC 61131-3 programs programmatically.

The server is not intended to be invoked directly from a terminal. An
MCP-compatible client — such as Claude Desktop, Cline, or Claude Code —
launches :program:`ironplcmcp` as a subprocess, writes JSON-RPC requests
to its standard input, and reads responses from its standard output. Log
output is written to standard error.

The server takes no command-line options. All per-request parameters
(source files, compiler dialect, feature flags) are supplied by the
client in each tool call. See :doc:`tools` for the full catalog.

The server exits with status 0 when the client disconnects cleanly, or
with a non-zero status and an error message on standard error if the
transport fails.

Configuration
=============

Configure your MCP client to launch :program:`ironplcmcp`. The command
must be on the ``PATH`` used by the client. Example entry for Claude
Desktop's :file:`claude_desktop_config.json`:

.. code-block:: json

   {
     "mcpServers": {
       "ironplc": {
         "command": "ironplcmcp"
       }
     }
   }

For step-by-step configuration of Claude Desktop, Cline, and Claude Code,
see
:doc:`/how-to-guides/ai-agents/write-plc-programs-with-an-ai-agent`.

Exit Status
===========

``0``
   The client disconnected cleanly.

non-zero
   The stdio transport failed or the server could not start. An error
   message is printed to standard error.

See Also
========

* :doc:`overview` --- MCP server overview
* :doc:`tools` --- Tool reference
* :doc:`/how-to-guides/ai-agents/write-plc-programs-with-an-ai-agent`
  --- Configuring an AI agent to use IronPLC
* :doc:`/reference/compiler/ironplcc` --- IronPLC compiler
