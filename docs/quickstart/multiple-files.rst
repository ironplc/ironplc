============================
Working with Multiple Files
============================

As your IEC 61131-3 application grows, you will want to organize your code
across multiple files. IronPLC combines all files into a single unit, so
you can split your application however you like.

--------------------------------------
Split the Application
--------------------------------------

Right now, :file:`main.st` contains both the program and the configuration.
Let's separate them.

First, edit :file:`main.st` so it contains **only** the program:

.. code-block::
   :caption: main.st — Program only
   :name: main-only

   PROGRAM main
      VAR
         Button : BOOL;
         Buzzer : BOOL;
         PulseTimer : TON;
      END_VAR

      PulseTimer(IN := NOT Button, PT := T#500ms);
      Buzzer := PulseTimer.Q;

   END_PROGRAM

Next, create a new file for the configuration:

#. In the main menu, select :menuselection:`File --> New File...`.
#. In the :guilabel:`New File...` dialog, select the :menuselection:`Structured Text File` option.
#. Enter the following code:

   .. code-block::
      :caption: config.st — Configuration only
      :name: config-only

      CONFIGURATION config
         RESOURCE res ON PLC
            TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
            PROGRAM plc_task_instance WITH plc_task : main;
         END_RESOURCE
      END_CONFIGURATION

#. Save the file with the name :file:`config.st`.

The IronPLC extension checks all :file:`.st` files in the workspace
together, so it will still validate that the configuration references a
valid program.

--------------------------------------
Compile with Multiple Files
--------------------------------------

When compiling multiple files, pass all of them to the compiler:

.. code-block:: shell
   :caption: Compile Multiple Files

   ironplcc compile main.st config.st --output main.iplc

The compiled output is the same — IronPLC merges all source files before
compiling.

--------------------------------------
Why Split Files?
--------------------------------------

For a small example like this, splitting may seem unnecessary. But in
real-world projects, separating programs from configuration has clear
benefits:

- **Reuse** — the same program can be referenced from different
  configurations (for example, testing vs. production).
- **Organization** — each file has a single responsibility.
- **Collaboration** — different team members can work on different files.

IronPLC does not impose any naming conventions on your files. Use whatever
structure makes sense for your project.

--------------------------------------
Next Steps
--------------------------------------

You now have a complete, multi-file IEC 61131-3 application. In the final
chapter, you will learn how to connect your program to physical hardware
inputs and outputs.

Continue to :doc:`compiling-and-running`.
