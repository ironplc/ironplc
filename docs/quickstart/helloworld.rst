==================
Your First Program
==================

Now it's time to write, compile, and run your first IEC 61131-3 program.
By the end of this chapter, you will have a working doorbell program running
in the IronPLC virtual machine.

--------------------------------------
Create a Project Directory
--------------------------------------

Open a terminal and create a new folder for your project:

.. code-block:: shell
   :caption: Create Project Directory

   mkdir doorbell
   cd doorbell

Then open the folder in your development environment:

.. code-block:: shell

   code .

.. tip::

   If you are using Cursor, use ``cursor .`` instead of ``code .``.

--------------------------------------
Write the Program
--------------------------------------

In your development environment:

#. In the main menu, select :menuselection:`File --> New File...`.
#. In the :guilabel:`New File...` dialog, select the :menuselection:`Structured Text File` option.
#. Enter the following code into the :guilabel:`Editor`:

   .. code-block::
      :caption: main.st — Doorbell Program
      :name: doorbell-program

      PROGRAM main
         VAR
            Button : BOOL;
            Buzzer : BOOL;
         END_VAR

         Buzzer := NOT Button;

      END_PROGRAM

#. Save the file with the name :file:`main.st`.

If the IronPLC extension is installed, you should see no errors highlighted
in the editor.

--------------------------------------
What This Program Does
--------------------------------------

Let's break it down:

- :code:`PROGRAM main` ... :code:`END_PROGRAM` defines a **program** named
  ``main``. A program is the basic unit of control logic in IEC 61131-3,
  similar to a ``main`` function in other languages.

- :code:`VAR` ... :code:`END_VAR` declares two **variables** of type
  :code:`BOOL`. ``Button`` represents the sensor input and ``Buzzer``
  represents the actuator output.

- ``Buzzer := NOT Button;`` is an **assignment statement**. The ``:=``
  operator assigns the value on the right to the variable on the left.
  When ``Button`` is ``FALSE`` (not pressed), ``Buzzer`` is ``TRUE``
  (sounding).

--------------------------------------
Open a Terminal
--------------------------------------

The next steps use the IronPLC command line tools. Open a terminal inside
your development environment:

- In Visual Studio Code or Cursor, select
  :menuselection:`Terminal --> New Terminal` from the main menu.

The terminal should open in your :file:`doorbell` project directory.

--------------------------------------
Check the Program
--------------------------------------

Run the following command to check your program for errors:

.. code-block:: shell
   :caption: Check Syntax

   ironplcc check main.st

On success, the command produces no output. If there are errors, IronPLC
prints diagnostics with the file name, line number, and a description of
the problem.

.. include:: ../includes/requires-compiler.rst

--------------------------------------
Compile the Program
--------------------------------------

Compile your program into a bytecode container:

.. code-block:: shell
   :caption: Compile to Bytecode

   ironplcc compile main.st --output main.iplc

On success, the command creates the :file:`main.iplc` file. This file
contains the compiled bytecode that the IronPLC virtual machine can execute.

--------------------------------------
Run the Program
--------------------------------------

Run the compiled program in the IronPLC virtual machine:

.. code-block:: shell
   :caption: Run and Inspect Variables

   ironplcvm run main.iplc --scans 1 --dump-vars

The ``--scans 1`` flag runs one scan cycle, and ``--dump-vars`` prints the
value of every variable after execution. You should see output like:

.. code-block:: text

   Button: FALSE
   Buzzer: TRUE

``Button`` starts as ``FALSE`` (the default for :code:`BOOL`), so
``NOT Button`` evaluates to ``TRUE``, and the buzzer sounds. This is
exactly the sense-control-actuate cycle in action — even though there is
no physical hardware connected yet.

--------------------------------------
Next Steps
--------------------------------------

You now have a working program, but it runs only once and exits. In the
next chapter, you will configure the application to run on a repeating
schedule and add a timer to make the buzzer pulse automatically.

Continue to :doc:`configuring`.
