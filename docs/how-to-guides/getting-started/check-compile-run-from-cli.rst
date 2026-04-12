==============================================
Check, Compile, and Run from the Command Line
==============================================

The IronPLC editor extension can check, compile, and run programs directly
in the editor. If you prefer the command line — for example, to integrate
with a CI/CD pipeline or a build script — this guide shows how to use the
:program:`ironplcc` and :program:`ironplcvm` tools directly.

.. include:: ../../includes/requires-compiler.rst

--------------------------------------
Open a Terminal
--------------------------------------

Open a terminal inside your development environment:

- In Visual Studio Code or Cursor, select
  :menuselection:`Terminal --> New Terminal` from the main menu.

The terminal should open in your project directory.

--------------------------------------
Check a Program
--------------------------------------

Run the following command to check your program for errors:

.. code-block:: shell

   ironplcc check main.st

On success, the command produces no output. If there are errors, IronPLC
prints diagnostics with the file name, line number, and a description of
the problem.

--------------------------------------
Compile a Program
--------------------------------------

Compile your program into a bytecode container:

.. code-block:: shell

   ironplcc compile main.st --output main.iplc

On success, the command creates the :file:`main.iplc` file. This file
contains the compiled bytecode that the IronPLC virtual machine can execute.

--------------------------------------
Run a Compiled Program
--------------------------------------

Run the compiled program in the IronPLC virtual machine:

.. code-block:: shell

   ironplcvm run main.iplc --scans 1 --dump-vars

The ``--scans 1`` flag runs one scan cycle, and ``--dump-vars`` prints the
value of every variable after execution. You should see output like:

.. code-block:: text

   Button: FALSE
   Buzzer: TRUE

--------------------------------------
Compile and Run with Multiple Files
--------------------------------------

When compiling multiple files, pass all of them to the compiler:

.. code-block:: shell

   ironplcc compile main.st config.st --output main.iplc

The compiled output is the same — IronPLC merges all source files before
compiling.

--------------------------------------
Run Multiple Scan Cycles
--------------------------------------

To observe how state changes over time, increase the number of scan cycles.
For example, with a program that uses a timer:

.. code-block:: shell

   ironplcc compile main.st --output main.iplc
   ironplcvm run main.iplc --scans 10 --dump-vars

You should see output showing the timer's state:

.. code-block:: text

   Button: FALSE
   Buzzer: TRUE
   PulseTimer.IN: TRUE
   PulseTimer.PT: T#500ms
   PulseTimer.Q: TRUE
   PulseTimer.ET: T#1000ms

The timer's elapsed time (``ET``) shows how long it has been running.
After enough scan cycles, ``Q`` becomes ``TRUE`` and the buzzer turns on.

--------------------------------------
See Also
--------------------------------------

- :doc:`/reference/compiler/ironplcc` — full :program:`ironplcc` command
  reference
- :doc:`/reference/runtime/ironplcvm` — full :program:`ironplcvm` command
  reference
- :doc:`/quickstart/index` — quickstart tutorial (uses the editor instead of
  the command line)
