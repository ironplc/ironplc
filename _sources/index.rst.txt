.. image:: images/banner.svg
   :align: center

IronPLC is an open-source toolchain for checking and compiling IEC 61131-3
programs. It integrates into Visual Studio Code to provide auto-completion,
syntax highlighting, and real-time error checking as you type. It also
includes a command-line compiler and an early-stage runtime.

IronPLC reads Structured Text, PLCopen XML (Beremiz), and TwinCAT 3 source
files, so you can use it alongside your existing PLC development environment.

The long-term vision is to become a full development environment for
building IEC 61131-3 based control systems on off-the-shelf embedded
computers (SoftPLCs). That goal is ambitious and IronPLC is still early in
that journey — but there is plenty you can do with it today. IronPLC is MIT
licensed and we'd love for you to give it a try, share feedback, or
contribute.

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

.. toctree::
   :maxdepth: 2
   :hidden:

   Quick start <quickstart/index>
   How-to guides <how-to-guides/index>
   Explanation <explanation/index>
   Reference <reference/index>
