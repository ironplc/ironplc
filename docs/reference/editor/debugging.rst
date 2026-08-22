=========
Debugging
=========

.. include:: /includes/debugging-in-development.rst

The IronPLC extension debugs Structured Text programs: set breakpoints on
source lines, step through the code, and inspect variables while the program
is paused.

Debugging is driven by :program:`ironplcdap`, which installs alongside the
:program:`ironplcc` compiler. There is no separate debug extension to install.
See :doc:`/reference/runtime/ironplcdap` for the server itself.

Before You Start
================

Debugging needs three things:

* **The IronPLC compiler.** The extension discovers :program:`ironplcc` and
  finds :program:`ironplcdap` beside it. See :doc:`problems/E0007` if the
  server cannot be found.
* **A program that compiles on its own.** The debugger compiles the single
  file you point it at. A file that references POUs or types defined in
  sibling files does not compile in isolation --- compile the project first
  and debug the container instead. See `Debug a Compiled Container`_.
* **A single program instance.** The configuration must declare exactly one
  ``PROGRAM ... WITH ...`` instance. A program that declares more is refused
  at launch, because a breakpoint would stop the first instance to reach it
  while the others still hold state from the previous scan. See
  :doc:`/reference/runtime/problems/V6010`.

Starting a Session
==================

To debug the file in the active editor:

#. Open a Structured Text file.
#. Set a breakpoint by clicking the gutter to the left of a line number.
#. Press :kbd:`F5`.

No :file:`launch.json` is required. When you press :kbd:`F5` with no debug
configuration, the extension debugs the active file.

The extension compiles the file to a temporary ``.iplc`` container, starts
:program:`ironplcdap`, and runs the program. Compiler output appears in the
:guilabel:`IronPLC Debug` output channel
(:menuselection:`View --> Output --> IronPLC Debug`).

Launch Configuration
====================

To keep a configuration, create a :file:`.vscode/launch.json` from the
:guilabel:`Run and Debug` view. The extension supplies a starting
configuration:

.. code-block:: json
   :caption: .vscode/launch.json

   {
     "version": "0.2.0",
     "configurations": [
       {
         "type": "ironplc",
         "request": "launch",
         "name": "IronPLC: Debug Active File",
         "program": "${file}",
         "stopOnEntry": false
       }
     ]
   }

Attributes
----------

.. list-table::
   :header-rows: 1
   :widths: 20 15 65

   * - Attribute
     - Type
     - Description
   * - ``program``
     - string
     - **Required.** Path to a Structured Text source file or a compiled
       ``.iplc`` container. A source file is compiled before the session
       starts; a container is launched as-is. Defaults to ``${file}``, the
       active editor's file.
   * - ``stopOnEntry``
     - boolean
     - Pause before the first scan cycle begins. Defaults to ``false``.
   * - ``scanLimit``
     - number
     - Stop the session after this many scan cycles. ``0`` means unlimited.
       See `Limiting a Run`_.

Only the ``launch`` request is supported. There is no ``attach``
configuration: the debugger starts the program it debugs.

A ``program`` that is neither a source file nor a ``.iplc`` container is
rejected before the session starts. See :doc:`problems/E0005`.

Debug a Compiled Container
--------------------------

Point ``program`` at a container to debug a whole project rather than a
single file:

.. code-block:: json

   {
     "type": "ironplc",
     "request": "launch",
     "name": "IronPLC: Debug Project",
     "program": "${workspaceFolder}/myproject.iplc"
   }

Produce the container with the :doc:`build task <build-tasks>` or with
:program:`ironplcc compile . -o myproject.iplc`. Breakpoints still bind to
your source files: the container records which file each line came from.

Rebuild the container after changing the source. The debugger launches the
container as it finds it and does not recompile it.

Limiting a Run
--------------

A PLC program does not finish. It scans until something stops it, so a debug
session with no breakpoint runs until you press :guilabel:`Stop`.

Set ``scanLimit`` to end the session after a fixed number of scan cycles:

.. code-block:: json

   {
     "type": "ironplc",
     "request": "launch",
     "name": "IronPLC: Debug One Scan",
     "program": "${file}",
     "scanLimit": 1
   }

Breakpoints
===========

Click the gutter to the left of a line number to set a breakpoint, or press
:kbd:`F9` on the current line. Set breakpoints before you launch or while the
program is paused.

Breakpoints bind to lines that generate code. A breakpoint on a blank line, a
comment, or a declaration moves down to the next line that does, and the dot
in the gutter moves with it to show where the breakpoint actually bound.

A breakpoint that cannot bind --- on ``END_IF``, on ``END_PROGRAM``, or past
the end of the code --- stays unverified, and the editor shows it as a hollow
dot. The session still runs; that breakpoint never pauses it.

A breakpoint in a scan-cycle program pauses *every* scan, not once. Continuing
from a breakpoint runs to the same breakpoint on the next cycle.

.. note::

   Breakpoints are line-level and unconditional. Conditional breakpoints, hit
   counts, logpoints, function breakpoints, data breakpoints, and inline
   (column) breakpoints are not supported. A breakpoint set on a column within
   a line binds to the whole line.

Execution Control
=================

While the program is paused:

.. list-table::
   :header-rows: 1
   :widths: 30 20 50

   * - Action
     - Shortcut
     - Behavior
   * - :guilabel:`Continue`
     - :kbd:`F5`
     - Resume until the next breakpoint, the scan limit, or the program ends.
   * - :guilabel:`Step Over`
     - :kbd:`F10`
     - Run the current line, including any call it makes, and stop on the next.
   * - :guilabel:`Step Into`
     - :kbd:`F11`
     - Stop on the first line of the function or function block being called.
   * - :guilabel:`Step Out`
     - :kbd:`Shift+F11`
     - Run to the end of the current POU and stop in its caller.
   * - :guilabel:`Stop`
     - :kbd:`Shift+F5`
     - End the session.

.. note::

   :guilabel:`Pause` and :guilabel:`Restart` are not supported. The debug
   server services requests only when the program is stopped, so a running
   program cannot be interrupted from the toolbar --- set a breakpoint before
   you launch, or use ``scanLimit``. To restart, stop the session and launch
   again.

.. note::

   The :guilabel:`Step Scan Cycle` button on the debug toolbar is not yet
   implemented. Pressing it reports that the request is not supported. Use
   :guilabel:`Continue` to advance to the next scan, and watch ``scanCount``
   in the :guilabel:`Runtime` scope to see the cycles pass.

Inspecting Variables
====================

The :guilabel:`Variables` view shows two scopes while the program is paused.

Program
-------

The variables your program declares, by name and type:

.. code-block:: text

   Counter : DINT = 42
   Running : BOOL = TRUE
   Label   : STRING = 'ready'

Values refresh at every stop.

Runtime
-------

State the virtual machine owns rather than your program:

.. list-table::
   :header-rows: 1
   :widths: 20 15 65

   * - Variable
     - Type
     - Description
   * - ``scanCount``
     - ``ULINT``
     - The number of scan cycles the program has completed. It increments as
       you continue, which is how you tell one scan from the next.

.. note::

   Values are read-only. Setting a variable while paused, forcing a value, and
   watch expressions are not supported, and expressions typed into the Debug
   Console are not evaluated. To change what the program does, edit the source
   and launch again.

   Forcing is absent rather than merely missing. In a PLC, forcing a variable
   means holding it at a value across scans, overriding what the program
   writes. A value set only while paused is overwritten on the next scan, a
   few milliseconds later, which looks like a debugger that does not work.

.. note::

   Two kinds of value do not render yet. A ``WSTRING`` shows
   ``<not available>``. A function block instance shows its name and type ---
   ``PulseTimer  TON  0`` --- but not its fields, so the ``ET`` of a ``TON``
   cannot be read here. Read the variable the function block writes to
   instead.

Program Time
============

The debugger drives the clock itself. Program time advances by one
millisecond for every scan cycle, rather than following the real clock, so a
``TON`` declared ``PT := T#500ms`` elapses on scan 500 no matter how long you
spend paused.

This makes a debug session repeatable: the same program stops at the same
breakpoint with the same values every time. It also means a timer under the
debugger measures scans rather than seconds. Outside the debugger,
:doc:`/reference/runtime/ironplcvm` uses the real clock.

Call Stack
==========

The :guilabel:`Call Stack` view names each frame by its POU and highlights
the paused line in the editor. Stepping into a function or function block
pushes a frame; stepping out pops it.

When the Program Traps
======================

A runtime error --- a division by zero, an out-of-range array index --- stops
the program at the failing instruction and reports the problem code. The
session stays open so you can inspect the state that caused it: the
:guilabel:`Variables` and :guilabel:`Call Stack` views still work. Execution
cannot resume from a trap; stop the session and launch again.

See :doc:`/reference/runtime/problems/index` for the problem codes the
runtime reports.

Settings
========

``ironplc.dapServerPath`` overrides the discovery of the debug server. See
:doc:`settings`.

Supported Languages
===================

The debugger is registered for Structured Text (:file:`.st`) and TwinCAT POU
(:file:`.TcPOU`) files. Breakpoints can be set in both.

See Also
========

* :doc:`/quickstart/debugging` --- debug a program for the first time
* :doc:`/how-to-guides/getting-started/debug-a-program` --- task recipes
* :doc:`/reference/runtime/ironplcdap` --- the debug server
* :doc:`problems/index` --- extension problem codes
