====================
MCP Server Reference
====================

The program :program:`ironplcmcp` (:program:`ironplcmcp.exe` on Windows) is
IronPLC's `Model Context Protocol <https://modelcontextprotocol.io/>`_ server.
It exposes the IronPLC compiler's parsing, semantic analysis, and code generation
capabilities as MCP tools so an AI coding agent can author, validate, and
compile IEC 61131-3 programs without invoking :program:`ironplcc` directly.

For an end-to-end walkthrough of connecting an AI agent to IronPLC, see
:doc:`/how-to-guides/ai-agents/write-plc-programs-with-an-ai-agent`.

.. toctree::
   :maxdepth: 1

   Overview <overview>
   Command Reference <ironplcmcp>
   Tool Reference <tools>
