=====================================
IronPLC and the IEC 61131-3 Ecosystem
=====================================

This page explains where IronPLC fits in the broader world of PLC
development tools.

--------------------------------------
The Traditional PLC Workflow
--------------------------------------

Most PLC manufacturers ship an integrated development environment (IDE) tied
to their hardware. For example:

- **Beckhoff** provides TwinCAT, which runs inside Visual Studio.
- **Siemens** provides TIA Portal for its S7 family.
- **Codesys** provides a vendor-neutral IDE that many smaller manufacturers
  rebrand (including the open-source **Beremiz** project).

These tools handle everything: editing, compiling, downloading to hardware,
debugging, and visualization. They are powerful, but they are also
proprietary, expensive, and locked to specific hardware.

--------------------------------------
What IronPLC Does Today
--------------------------------------

IronPLC is an open-source toolchain for working with IEC 61131-3 code. Today
it provides:

- **A compiler** (:program:`ironplcc`) that parses and checks IEC 61131-3
  programs for correctness. It catches syntax errors, type mismatches, and
  other problems before you ever download code to a PLC.
- **A VS Code extension** that provides auto-completion, syntax highlighting
  and real-time error checking as you type.
- **A runtime** (:program:`ironplcvm`) that can execute simple compiled
  programs. The runtime is in early development and supports only a limited
  subset of the language.

IronPLC reads several source formats:

- **Structured Text** (:file:`.st` files) — the native text format
- **PLCopen XML** (:file:`.xml`, :file:`plc.xml`) — used by Beremiz and other
  PLCopen-compatible tools
- **TwinCAT** (:file:`.TcPOU`, :file:`.TcGVL`, :file:`.plcproj`) — used by
  Beckhoff TwinCAT 3

This means you can point IronPLC at an existing project from Beremiz or
TwinCAT and get a second opinion on your code without changing your workflow.

--------------------------------------
What IronPLC Does Not Do (Yet)
--------------------------------------

IronPLC is a young project. Some things it cannot do today:

- **Run on real PLC hardware.** The runtime currently targets a virtual
  machine, not physical I/O.
- **Support the full IEC 61131-3 language.** Many features are parsed and
  checked but code generation covers only a small subset.
- **Replace your existing IDE.** IronPLC is a complement to your existing
  tools, not a replacement.

The long-term vision is to become a full development environment for building
IEC 61131-3 based control systems that run on off-the-shelf embedded
computers (sometimes called SoftPLCs). That goal is ambitious, and
contributions are welcome.

--------------------------------------
How IronPLC Relates to Other Tools
--------------------------------------

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Tool
     - Relationship to IronPLC
   * - **Beremiz**
     - An open-source PLC IDE. IronPLC can read Beremiz project files
       (:file:`plc.xml`) to provide additional checking. See
       :doc:`/how-to-guides/beremiz/check-beremiz-projects`.
   * - **TwinCAT**
     - Beckhoff's PLC IDE. IronPLC can read TwinCAT project files to provide
       additional checking. See
       :doc:`/how-to-guides/twincat/check-twincat-projects`.
   * - **Codesys**
     - A widely used commercial PLC IDE. IronPLC does not currently read
       Codesys project files, but Codesys can export to PLCopen XML.
   * - **OpenPLC**
     - An open-source PLC runtime. IronPLC and OpenPLC have different goals:
       OpenPLC focuses on running programs on hardware, while IronPLC focuses
       on checking and compiling code.
