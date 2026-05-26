.. meta::
   :description: IronPLC is an open-source IEC 61131-3 toolchain with an extension for your development environment, command-line compiler, and browser-based playground for Structured Text programming.

.. |logo| image:: _static/ironplc-banner.svg
   :alt: IronPLC

======
|logo|
======

IronPLC is a compiler, runtime, Visual Studio Code editor extension,
and MCP server for writing and running IEC 61131-3 programs.
IronPLC reads Structured Text, PLCopen XML and other vendor files directly,
so you can use IronPLC alongside your existing PLC development environment.

.. figure:: /images/screenshots/quickstart-animation.png
   :alt: VS Code showing an IEC 61131-3 Structured Text file with syntax highlighting
   :width: 600px

   Structured Text with syntax highlighting in Visual Studio Code.

IronPLC supports most of IEC 61131-3 edition 2 and parts of edition 3
but doesn't yet provide I/O mapping or debugging capabilities. Still,
there is plenty you can do with IronPLC today including running
code using the `IronPLC Playground <https://playground.ironplc.com>`_,
authoring code with an AI agent using IronPLC's Model Context Protocol
(MCP) server, and running applications locally.

IronPLC is free (subject to MIT license terms) and works on Windows,
macOS and Linux.

.. grid:: 2

    .. grid-item-card::  Tutorials
        :link: quickstart/index
        :link-type: doc

        New to IEC 61131-3 or IronPLC? Start here with a step-by-step
        guide that builds up from nothing.

    .. grid-item-card::  How-to guides
        :link: how-to-guides/index
        :link-type: doc

        Practical guides for specific tasks — whether you are getting
        started, coming from Beremiz, or coming from TwinCAT.

.. grid:: 2

    .. grid-item-card::  Explanation
        :link: explanation/index
        :link-type: doc

        Understand the concepts behind IEC 61131-3: the scan cycle, program
        organization, Structured Text, and how I/O works.

    .. grid-item-card::  Reference
        :link: reference/index
        :link-type: doc

        Technical reference for the compiler, editor extension, and runtime.

.. grid:: 1

    .. grid-item-card::  Playground
        :link: https://playground.ironplc.com
        :link-type: url

        Try IronPLC in your browser — no installation needed. Write, compile,
        and run IEC 61131-3 programs directly in the
        `IronPLC Playground <https://playground.ironplc.com>`_.

.. toctree::
   :maxdepth: 2
   :hidden:

   Quick start <quickstart/index>
   How-to guides <how-to-guides/index>
   Explanation <explanation/index>
   Reference <reference/index>
   Trademarks <trademarks>
