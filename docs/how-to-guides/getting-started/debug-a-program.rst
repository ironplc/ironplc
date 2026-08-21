=================
Debug a Program
=================

.. include:: /includes/debugging-in-development.rst

This guide shows how to pause a running program, step through it, and read
its variables from your editor. For the full launch configuration and the
current limits, see :doc:`/reference/editor/debugging`.

.. include:: /includes/requires-compiler.rst

--------------------------------------
Debug the File You Are Editing
--------------------------------------

#. Open a Structured Text file.
#. Click the gutter to the left of a line number to set a breakpoint. A red
   dot appears.
#. Press :kbd:`F5`.

The program compiles, starts, and stops on the line you marked. The
:guilabel:`Variables` view shows your variables by name and type, and the
:guilabel:`Call Stack` view shows which POU you are in.

Press :kbd:`F10` to run the current line and stop on the next one, or
:kbd:`F5` to continue.

No :file:`launch.json` is needed. To keep a configuration, open the
:guilabel:`Run and Debug` view and select
:guilabel:`create a launch.json file`.

.. note::

   The breakpoint dot may move down a line when the session starts.
   Breakpoints bind to lines that generate code, so a breakpoint on a blank
   line, a comment, or a declaration moves to the next line that does.

--------------------------------------
Stop Before the First Scan
--------------------------------------

To inspect the program's initial values before any code runs, set
``stopOnEntry``:

.. code-block:: json
   :caption: .vscode/launch.json

   {
     "type": "ironplc",
     "request": "launch",
     "name": "IronPLC: Stop on Entry",
     "program": "${file}",
     "stopOnEntry": true
   }

The program pauses before the first scan cycle. This is the state your
declared initial values produce, before any assignment has run.

--------------------------------------
Stop After a Fixed Number of Scans
--------------------------------------

A PLC program does not end on its own. Without a breakpoint, a debug session
scans until you stop it.

Set ``scanLimit`` to end the session after a fixed number of cycles:

.. code-block:: json
   :caption: .vscode/launch.json

   {
     "type": "ironplc",
     "request": "launch",
     "name": "IronPLC: One Scan",
     "program": "${file}",
     "scanLimit": 1
   }

--------------------------------------
Debug a Multi-File Project
--------------------------------------

The debugger compiles the single file you point it at. A file that uses
POUs or types declared in other files does not compile on its own, and the
session stops with :doc:`/reference/editor/problems/E0006`.

Compile the project first, then debug the container it produces:

#. Compile the whole project:

   .. code-block:: shell

      ironplcc compile . -o myproject.iplc

   You can also use the :doc:`build task </reference/editor/build-tasks>`,
   which does the same thing.

#. Point ``program`` at the container:

   .. code-block:: json
      :caption: .vscode/launch.json

      {
        "type": "ironplc",
        "request": "launch",
        "name": "IronPLC: Debug Project",
        "program": "${workspaceFolder}/myproject.iplc"
      }

Breakpoints still work in your source files. The container records which
file each line came from, so the debugger matches your breakpoints to the
right file.

Recompile after every source change. The debugger runs the container as it
finds it.

--------------------------------------
Debug a TwinCAT POU
--------------------------------------

Breakpoints work in :file:`.TcPOU` files the same way they do in
:file:`.st` files.

A POU that references other POUs, global variable lists, or library
functions does not compile on its own. Compile the project and debug the
container, as above:

.. code-block:: shell

   ironplcc compile --dialect twincat path/to/my-solution --output myproject.iplc

The ``--dialect twincat`` option matters here: the debugger compiles a single
file with default options, so a POU that uses TwinCAT extensions has to be
compiled ahead of time and debugged as a container.

See :doc:`/how-to-guides/twincat/run-twincat-projects` for compiling a
TwinCAT project.

--------------------------------------
Watch the Scan Cycle
--------------------------------------

A breakpoint in a PLC program pauses *every* scan, not once. To tell one
scan from the next, expand the :guilabel:`Runtime` scope in the
:guilabel:`Variables` view and watch ``scanCount``.

Press :kbd:`F5` to continue. The program runs to the end of the scan, starts
the next one, and stops on the same breakpoint with ``scanCount`` one
higher. Anything your program accumulates --- a counter, a timer --- moves
with it.

:doc:`/explanation/debugging-a-scan-cycle` explains what this means for the
values you are looking at.

--------------------------------------
When the Debugger Does Not Start
--------------------------------------

.. list-table::
   :header-rows: 1
   :widths: 45 55

   * - Symptom
     - Cause
   * - "No program specified to debug"
     - No file to debug. See :doc:`/reference/editor/problems/E0004`.
   * - "Program is not debuggable"
     - ``program`` is neither a source file nor a ``.iplc`` container. See
       :doc:`/reference/editor/problems/E0005`.
   * - The session stops with compiler errors
     - The file did not compile. Check the :guilabel:`IronPLC Debug` output
       channel. See :doc:`/reference/editor/problems/E0006`.
   * - "Debug server not found"
     - :program:`ironplcdap` was not found next to the compiler. Set
       ``ironplc.dapServerPath``. See
       :doc:`/reference/editor/problems/E0007`.
   * - "Compile with debug info enabled"
     - The container has no debug information. Recompile it with
       :program:`ironplcc`. See
       :doc:`/reference/runtime/problems/V6009`.
   * - The program declares more than one instance
     - Debugging supports a single program instance. See
       :doc:`/reference/runtime/problems/V6010`.

See :doc:`/how-to-guides/troubleshoot-editor` for other extension problems.

--------------------------------------
Next Steps
--------------------------------------

* :doc:`/quickstart/debugging` --- a worked example that finds a real bug
* :doc:`/reference/editor/debugging` --- every launch attribute and limit
* :doc:`/explanation/debugging-a-scan-cycle` --- what pausing means in a PLC
