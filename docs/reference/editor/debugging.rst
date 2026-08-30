=========
Debugging
=========

.. include:: /includes/debugging-in-development.rst

The IronPLC extension debugs Structured Text programs: set breakpoints on
source lines, step through the code, and inspect variables while the program
is paused.

Debugging is driven by :program:`ironplcvmd`, which installs alongside the
:program:`ironplcc` compiler. There is no separate debug extension to install.
See :doc:`/reference/runtime/ironplcvmd` for the server itself.

Before You Start
================

Debugging needs two things:

* **The IronPLC compiler.** The extension discovers :program:`ironplcc` and
  finds :program:`ironplcvmd` beside it. See :doc:`problems/E0007` if the
  server cannot be found.
* **A single program instance.** The configuration must declare exactly one
  ``PROGRAM ... WITH ...`` instance. A program that declares more is refused
  at launch. See :doc:`/reference/runtime/problems/V6010`.

Starting a Session
==================

To debug the file in the active editor:

#. Open a Structured Text file.
#. Set a breakpoint by clicking the gutter to the left of a line number.
#. Press :kbd:`F5`.

No :file:`launch.json` is required. When you press :kbd:`F5` with no debug
configuration, the extension debugs the active file.

The extension compiles the file to a temporary ``.iplc`` container, starts
:program:`ironplcvmd`, and runs the program. Compiler output appears in the
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
     - Stop the session after this many scan cycles. Must be a whole number of
       at least ``1``; omit it to run without a bound. See `Limiting a Run`_.

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

Leave ``scanLimit`` out of the configuration to run without a bound --- there
is no value that means unlimited. ``0`` and ``-1`` are rejected before the
session starts (see :doc:`/reference/runtime/problems/V6011`) rather than
treated as either extreme.

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
   * - :guilabel:`Step Scan Cycle`
     - None
     - Run the rest of the current scan cycle and stop at the start of the
       next one.
   * - :guilabel:`Stop`
     - :kbd:`Shift+F5`
     - End the session.

Scan Stepping
-------------

:guilabel:`Step Scan Cycle` is on the debug toolbar and in the Command
Palette. It is the scan-cycle equivalent of :guilabel:`Step Over`: one press
advances the program by exactly one cycle, no matter how many lines that
takes.

The stop lands on the first line of the *next* cycle. The cycle you stepped
has finished --- its outputs are written and ``scanCount`` in the
:guilabel:`Runtime` scope has gone up by one --- so the values you see are
that cycle's results.

A breakpoint reached partway through the cycle stops there instead, the same
way one reached during :guilabel:`Step Over` does. The scan step ends at that
breakpoint; press :guilabel:`Step Scan Cycle` again to run out the rest of the
cycle.

If the cycle you step is the last one allowed by ``scanLimit``, the session
ends rather than stopping again.

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
   * - ``systemUptime``
     - ``LINT``
     - The virtual machine's monotonic clock, in milliseconds, as of the start
       of the scan cycle you are paused in. This is the value a program reads
       from ``__SYSTEM_UP_LTIME``, and the debugger shows it whether or not the
       program was compiled with ``--allow-system-uptime-global``.

.. note::

   Values are read-only. Setting a variable while paused, forcing a value, and
   watch expressions are not supported, and expressions typed into the Debug
   Console are not evaluated. To change what the program does, edit the source
   and launch again.

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

``ironplc.debugServerPath`` overrides the discovery of the debug server. See
:doc:`settings`.

Supported Languages
===================

The debugger is registered for Structured Text (:file:`.st`) and TwinCAT POU
(:file:`.TcPOU`) files. Breakpoints can be set in both.

See Also
========

* :doc:`/quickstart/debugging` --- debug a program for the first time
* :doc:`/how-to-guides/getting-started/debug-a-program` --- task recipes
* :doc:`/reference/runtime/ironplcvmd` --- the debug server
* :doc:`problems/index` --- extension problem codes
