================================
The Sense-Control-Actuate Cycle
================================

In the previous chapter, you wrote a program that increments a counter. That
program works, but it does not interact with the outside world. In this
chapter, you will connect your program to inputs and outputs using a doorbell
example.

--------------------------------------
The Idea
--------------------------------------

Controllers normally operate as part of a **sense-control-actuate** cycle:

1. **Sense** — read inputs from sensors.
2. **Control** — evaluate logic to decide what to do.
3. **Actuate** — write outputs to actuators.

We'll illustrate this with a simple doorbell system. The system has a button
(the sensor) and a buzzer (the actuator).

.. figure:: button-buzzer.svg
   :width: 200

   Pressing the button triggers the buzzer.

We want the buzzer to sound when the button is pressed. To do that, our
program reads the button state and sets the buzzer accordingly.

.. note::

   A real doorbell does not need a PLC. This example is deliberately simple
   to illustrate IEC 61131-3 concepts.

--------------------------------------
Add Variables for I/O
--------------------------------------

Open the :file:`main.st` file from the previous chapter and replace its
contents with:

.. code-block::
   :caption: Doorbell Program
   :name: doorbell

   PROGRAM main
      VAR
         Button AT %IX1: BOOL;
         Buzzer AT %QX1: BOOL;
      END_VAR

      Buzzer := NOT Button;

   END_PROGRAM

Save the file. The IronPLC extension checks the file automatically — you
should see no errors.

--------------------------------------
What Changed
--------------------------------------

We replaced the counter with two new variables:

- ``Button AT %IX1 : BOOL`` — a Boolean **input** variable. The :code:`AT`
  keyword followed by ``%IX1`` maps this variable to a physical input
  address. The ``I`` means input, ``X`` means single bit, and ``1`` is the
  address number.

- ``Buzzer AT %QX1 : BOOL`` — a Boolean **output** variable. The ``Q``
  means output.

These are called **directly represented variables** because they are tied
to specific hardware I/O points. On a real PLC, ``%IX1`` would correspond
to a digital input pin (the button) and ``%QX1`` to a digital output pin
(the buzzer).

The statement ``Buzzer := NOT Button;`` assigns the logical inverse of
the button state to the buzzer. When the button is pressed (FALSE in this
wiring), the buzzer turns on (TRUE).

.. tip::

   For a complete explanation of the addressing format (``%IX``, ``%QX``,
   ``%MW``, etc.), see :doc:`/explanation/variables-and-io`.

--------------------------------------
Next Steps
--------------------------------------

You now have a program with inputs and outputs, but it is not yet a
complete IEC 61131-3 application. We need to tell the runtime *how often*
to run this program and *where* to run it. That's what configurations,
resources, and tasks are for.

Continue to :doc:`configuring`.
