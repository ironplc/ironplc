====================
Coming from TwinCAT
====================

These guides are for engineers who already use Beckhoff TwinCAT 3 and want
to use IronPLC alongside it.

IronPLC reads your TwinCAT solution as-is — the :file:`.sln`,
:file:`.tsproj`, and :file:`.plcproj` project files and the
:file:`.TcPOU`, :file:`.TcGVL`, :file:`.TcDUT`, and :file:`.TcIO` source
files that TwinCAT XAE created. Without changing your project or your
workflow, you can:

- **check** your project for problems and get a second opinion on your code,
- **run** your control logic on any computer — no TwinCAT runtime, license,
  or PLC hardware required, and
- keep using the **Beckhoff libraries** your project references, backed by
  IronPLC's bundled compatibility libraries.

.. toctree::
   :maxdepth: 1

   Check TwinCAT 3 Projects <check-twincat-projects>
   Run TwinCAT 3 Projects Without a PLC <run-twincat-projects>
   Use Beckhoff Libraries <use-beckhoff-libraries>

------------------------------------
What Works Today
------------------------------------

IronPLC checks and executes Structured Text (the language behind most
TwinCAT PLC code); graphical languages are not supported. Library support
is early: IronPLC bundles compatibility implementations for a growing set
of the most common ``Tc2_*`` libraries. See
:doc:`/reference/compiler/source-formats/twincat` for the supported file
formats and :doc:`/reference/compatibility-libraries/index` for exactly
which library functions are available.
