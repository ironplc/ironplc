========================
Connecting to Hardware
========================

So far, your doorbell program uses regular variables with no connection to
physical hardware. On a real PLC, variables are mapped to specific input
and output pins. In this chapter, you will learn how to make that
connection.

--------------------------------------
Directly Represented Variables
--------------------------------------

IEC 61131-3 uses the :code:`AT` keyword to bind a variable to a physical
I/O address. Open :file:`main.st` and update the variable declarations:

.. code-block::
   :caption: main.st — With Hardware Addresses
   :name: hardware-doorbell

   PROGRAM main
      VAR
         Button AT %IX1 : BOOL;
         Buzzer AT %QX1 : BOOL;
         PulseTimer : TON;
      END_VAR

      PulseTimer(IN := NOT Button, PT := T#500ms);
      Buzzer := PulseTimer.Q;

   END_PROGRAM

--------------------------------------
What Changed
--------------------------------------

We added :code:`AT` addresses to the ``Button`` and ``Buzzer`` variables:

- ``Button AT %IX1 : BOOL`` — a Boolean **input** variable. The ``I``
  means input, ``X`` means single bit, and ``1`` is the address number.
  On a real PLC, ``%IX1`` corresponds to a digital input pin connected
  to the button.

- ``Buzzer AT %QX1 : BOOL`` — a Boolean **output** variable. The ``Q``
  means output. On a real PLC, ``%QX1`` corresponds to a digital output
  pin connected to the buzzer.

These are called **directly represented variables** because they are tied
to specific hardware I/O points. The address format follows a pattern:

- ``%I`` — input, ``%Q`` — output, ``%M`` — memory
- ``X`` — single bit, ``B`` — byte, ``W`` — word (16-bit), ``D`` — double word (32-bit)
- The number is the address within that region

--------------------------------------
Check the Program
--------------------------------------

You can verify the updated program is correct:

.. code-block:: shell

   ironplcc check main.st config.st

The IronPLC checker validates the hardware addresses and ensures the
program is well-formed.

.. note::

   The IronPLC compiler does not yet support compiling programs with
   directly represented variables to bytecode. You can check these
   programs for correctness, but compiling and running them requires a
   future release. For now, remove the :code:`AT` addresses to compile
   and run in the virtual machine.

--------------------------------------
What You Have Learned
--------------------------------------

Over the course of this tutorial, you have:

1. **Installed** IronPLC and the development environment extension.
2. **Learned** the sense-control-actuate cycle that drives PLC programs.
3. **Written** a doorbell program with boolean logic and a timer.
4. **Run** the program directly from the editor.
5. **Configured** the application with a task schedule.
6. **Organized** the code across multiple files.
7. **Connected** variables to hardware I/O addresses.

--------------------------------------
Where to Go from Here
--------------------------------------

- :doc:`/explanation/index` — deepen your understanding of IEC 61131-3
  concepts.
- :doc:`/how-to-guides/index` — practical guides for specific tasks.
- :doc:`/reference/compiler/index` — full command and language reference.
