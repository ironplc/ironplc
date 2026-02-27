========
Overview
========

You can use the command line interface to check a file (and sets of files)
for correctness.

.. include:: ../../includes/requires-compiler.rst

--------------------------
Create a Project Directory
--------------------------

You'll start by making a directory to store your IEC 61131-3 code.
:program:`ironplcc` doesn't care where your code lives (and your
code can be in multiple directories), but creating a directory will
make it easy to work with your code.

Open a terminal and enter the commands in :ref:`Create Project Directory <compiler-create-directory>`
to make the :file:`ironplc-hello-world` directory.

.. code-block:: shell
   :caption: Create Project Directory
   :name: compiler-create-directory

   mkdir ~/ironplc-hello-world
   cd ~/ironplc-hello-world

-----------------------------
Create an IEC 61131-3 Program
-----------------------------

The next step is to create a source file for your IEC 61131-3 program.
:program:`ironplcc` doesn't care what your call your file(s), but it will
automatically detect file names with the :file:`.st` extension as IEC
61131-3 programs.

.. seealso::
   IronPLC also supports PLCopen XML and TwinCAT formats. See
   :doc:`source-formats/index` for all supported formats.

In the same terminal, enter the commands in :ref:`Create Hello World Program <compiler-create-hello-world>`
to create a program.

.. code-block:: shell
   :caption: Create Hello World Program
   :name: compiler-create-hello-world

   echo "PROGRAM main
      VAR
         Button AT %IX1: BOOL;
         Buzzer AT %QX1: BOOL;
      END_VAR

      Buzzer := NOT Button;

   END_PROGRAM

   CONFIGURATION config
      RESOURCE res ON PLC
         TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
         PROGRAM plc_task_instance WITH plc_task : main;
      END_RESOURCE
   END_CONFIGURATION" > main.st

---------------------------------
Check the Program for Correctness
---------------------------------

Finally, in the same terminal, run the commands in :ref:`Check Syntax <compiler-check-syntax>`
to check your program's syntax.

.. code-block:: shell
   :caption: Check Syntax
   :name: compiler-check-syntax

   ironplcc check main.st

On success, the command produces no output.

-------------------
Compile the Program
-------------------

.. warning::

   The compile command currently supports only trivial programs. Supported
   features include: ``PROGRAM`` declarations, ``INT`` variable declarations,
   assignment statements, integer literal constants, and the ``+`` (add)
   operator. Programs using other features will produce a code generation
   error.

You can compile a source file into a bytecode container (``.iplc``) file
using the ``compile`` command. Run the commands in
:ref:`Compile Program <compiler-compile-program>` to compile your program.

.. code-block:: shell
   :caption: Compile Program
   :name: compiler-compile-program

   ironplcc compile main.st --output main.iplc

On success, the command produces no output and creates the ``.iplc`` file
at the specified output path.

You can also use the short form ``-o`` for the output flag:

.. code-block:: shell

   ironplcc compile main.st -o main.iplc

----------------------
Execute the .iplc File
----------------------

To run a compiled ``.iplc`` file, use the IronPLC virtual machine runtime
:program:`ironplcvm`:

.. code-block:: shell

   ironplcvm run main.iplc

You can inspect variable values after execution by specifying a dump file:

.. code-block:: shell

   ironplcvm run main.iplc --dump-vars output.txt