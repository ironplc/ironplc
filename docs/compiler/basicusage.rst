===========
Basic Usage
===========

You can use the command line interface to check a file (and sets of files)
for correctness.

.. note::
   This section assumes you have installed the IronPLC Compiler. See :ref:`installation steps target`
   if you have not already installed the IronPLC Compiler.

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
   IronPLC supports multiple source formats including Text (``.st``, ``.iec``),
   PLCopen XML, and TwinCAT (``.TcPOU``, ``.TcGVL``, ``.TcDUT``). See
   :doc:`source-formats/index` for details on supported formats and their
   capabilities.

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

For now, that's it. Presently, IronPLC does not create runnable programs.