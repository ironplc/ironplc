======================
Compile to WebAssembly
======================

:program:`ironplcc` compiles a program to a WebAssembly logic module as well
as to a bytecode container. A logic module runs in any WebAssembly engine
that provides the runtime interface it imports: in a browser, on a PC with
wasmtime, or on a microcontroller with wasmi.

.. include:: ../../includes/requires-compiler.rst

--------------------------------------
Compile a Program
--------------------------------------

Add ``--target wasm`` to the compile command:

.. code-block:: shell

   ironplcc compile main.st --target wasm --output main.wasm

On success, the command creates two files:

* :file:`main.wasm`, the logic module;
* :file:`main.symbols.json`, its symbol map: the address, type and size of
  each variable, and the tasks and their programs.

The module also carries the symbol map in its ``plc.meta`` custom section,
so a runtime needs only the :file:`.wasm` file.

--------------------------------------
Name the Variables
--------------------------------------

The symbol map names each variable by its path in upper case:

* ``MAIN.COUNT`` for the variable ``count`` of the program instance ``main``;
* ``MAIN.TIMER.Q`` for the output ``Q`` of the function block instance
  ``timer``;
* ``LINE_SPEED`` for the global variable ``line_speed``.

Without a :code:`CONFIGURATION`, the program instance takes the name of the
program. With one, it takes the name of its :code:`PROGRAM ... WITH` line.

--------------------------------------
Run the Module
--------------------------------------

A runtime drives the module through its exports:

#. call ``plc_init`` once to set every variable to its initial value;
#. for each cycle, call ``plc_task_run`` with the number of the task in the
   symbol map, and provide the cycle time in nanoseconds through the
   imported ``plc_rt.now_ns`` function;
#. read and write variables in the exported ``memory`` at the addresses of
   the symbol map.

--------------------------------------
Bound the Time of a Cycle
--------------------------------------

Add ``--wasm-fuel`` to make the module count the instructions it executes.
The runtime sets the exported ``plc_fuel`` global before each call, and the
module traps when a cycle uses it up.

--------------------------------------
Limitations
--------------------------------------

The WebAssembly target computes the same values as the bytecode, with the
same representations: :code:`TIME` in milliseconds, dates in seconds. It
does not yet support:

* object-oriented function blocks (methods, :code:`EXTENDS`,
  :code:`IMPLEMENTS`);
* the trigonometric functions (:code:`SIN`, :code:`COS`, :code:`TAN`,
  :code:`ASIN`, :code:`ACOS`, :code:`ATAN`, :code:`ATAN2`);
* conversions between strings and numbers, and BCD conversions;
* sequential function charts.

A program that uses one of these produces
:doc:`P9999 </reference/compiler/problems/P9999>` and no module.
