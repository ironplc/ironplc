=======================
Compiling and Running
=======================

So far you have been writing code and letting the VS Code extension check it
for correctness. In this chapter, you will use the command line to compile
your program into a bytecode container and run it.

.. include:: ../includes/requires-compiler.rst

.. warning::

   The compile command currently supports only trivial programs. Supported
   features include: :code:`PROGRAM` declarations, :code:`INT` variable
   declarations, assignment statements, integer literal constants, and
   the ``+`` (add) operator. Programs using other features will produce a
   code generation error.

--------------------------------------
Check the Program
--------------------------------------

Open a terminal in your :file:`helloworld` directory and run:

.. code-block:: shell
   :caption: Check Syntax
   :name: qs-check-syntax

   ironplcc check main.st config.st

On success, the command produces no output. If there are errors, IronPLC
prints diagnostics with the file name, line number, and a description of
the problem.

--------------------------------------
Compile to Bytecode
--------------------------------------

To compile your source files into a bytecode container (:file:`.iplc` file),
run:

.. code-block:: shell
   :caption: Compile Program
   :name: qs-compile-program

   ironplcc compile main.st --output main.iplc

You can also use the short form ``-o`` for the output flag:

.. code-block:: shell

   ironplcc compile main.st -o main.iplc

On success, the command creates the :file:`.iplc` file at the specified path.

--------------------------------------
Run the Program
--------------------------------------

Use the IronPLC virtual machine runtime to execute the compiled program:

.. code-block:: shell
   :caption: Run Program
   :name: qs-run-program

   ironplcvm run main.iplc

You can inspect variable values after execution by specifying a dump file:

.. code-block:: shell

   ironplcvm run main.iplc --dump-vars output.txt

--------------------------------------
What You Have Learned
--------------------------------------

Over the course of this tutorial, you have:

1. **Installed** IronPLC and the VS Code extension.
2. **Written** a minimal IEC 61131-3 program.
3. **Connected** it to inputs and outputs with directly represented variables.
4. **Configured** the application with a task, resource, and configuration.
5. **Organized** the code across multiple files.
6. **Compiled** and **run** the program from the command line.

--------------------------------------
Where to Go from Here
--------------------------------------

- :doc:`/explanation/index` — deepen your understanding of IEC 61131-3
  concepts.
- :doc:`/how-to-guides/index` — practical guides for specific tasks.
- :doc:`/reference/compiler/index` — full command and language reference.
