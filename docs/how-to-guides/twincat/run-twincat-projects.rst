=====================================
Run TwinCAT 3 Projects Without a PLC
=====================================

This guide shows how to compile a Beckhoff TwinCAT 3 project and execute
its logic in the IronPLC virtual machine — on any computer, with no
TwinCAT runtime, no license, and no PLC hardware. This is useful for
testing control logic on a laptop or in a CI/CD pipeline.

.. include:: ../../includes/requires-compiler.rst

-------------------------------------------
Compile the Solution
-------------------------------------------

Compile your solution into a bytecode container: point
:program:`ironplcc` at the solution directory and select the ``twincat``
dialect:

.. code-block:: shell

   ironplcc compile --dialect twincat path/to/my-solution --output main.iplc

IronPLC discovers the sources through the :file:`.plcproj` project files,
activates any referenced Beckhoff libraries (see
:doc:`use-beckhoff-libraries`), and on success creates :file:`main.iplc` —
the compiled bytecode that the IronPLC virtual machine executes.

-------------------------------------------
Run the Compiled Program
-------------------------------------------

Run the container in the IronPLC virtual machine:

.. code-block:: shell

   ironplcvm run main.iplc --scans 1 --dump-vars

The ``--scans 1`` flag runs one scan cycle, and ``--dump-vars`` prints the
value of every variable after execution. For a program that converts a
commanded speed from degrees to radians, the output looks like:

.. code-block:: text

   commandedDegPerSec: 15
   commandedRadPerSec: 0.2617993877991494

Your ``MAIN`` program runs exactly as it would in each PLC cycle. To
observe state that evolves over time — timers, counters, edge triggers —
increase ``--scans``.

-------------------------------------------
Run in a CI/CD Pipeline
-------------------------------------------

Both tools return a non-zero exit code on failure, so the same commands
work as a pipeline gate:

.. code-block:: shell

   ironplcc check --dialect twincat . || exit 1
   ironplcc compile --dialect twincat . --output main.iplc || exit 1
   ironplcvm run main.iplc --scans 10 || exit 1

This catches problems on every commit — before the code ever reaches a
test rig.

-------------------------------------------
See Also
-------------------------------------------

- :doc:`/how-to-guides/getting-started/check-compile-run-from-cli` — the
  same workflow explained for plain IEC 61131-3 files
- :doc:`/reference/compiler/ironplcc` — full :program:`ironplcc` command
  reference
- :doc:`/reference/runtime/ironplcvm` — full :program:`ironplcvm` command
  reference
- :doc:`/explanation/execution-cycle` — how scan cycles work
