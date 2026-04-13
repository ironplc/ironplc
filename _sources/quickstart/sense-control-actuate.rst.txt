========================
How a PLC Program Works
========================

Before writing your first program, it helps to understand how a PLC
application is structured. This chapter introduces the core idea that every
PLC program is built around.

--------------------------------------
The Sense-Control-Actuate Cycle
--------------------------------------

Controllers operate in a continuous **sense-control-actuate** cycle:

1. **Sense** — read inputs from sensors (buttons, temperature probes, etc.).
2. **Control** — evaluate logic to decide what to do.
3. **Actuate** — write outputs to actuators (buzzers, motors, valves, etc.).

The runtime repeats this cycle on a fixed schedule — for example, every
100 milliseconds. Each repetition is called a **scan cycle**.

--------------------------------------
A Doorbell Example
--------------------------------------

A simple doorbell system illustrates the cycle. The system has a button
(the sensor) and a buzzer (the actuator):

.. figure:: button-buzzer.svg
   :width: 200

   Pressing the button triggers the buzzer.

In each scan cycle, the PLC:

1. **Senses** whether the button is pressed.
2. **Controls** — decides the buzzer should sound when the button is pressed.
3. **Actuates** — turns the buzzer on or off.

.. note::

   A real doorbell does not need a PLC. This example is deliberately simple
   to illustrate how PLC programs work.

--------------------------------------
What You Will Build
--------------------------------------

In this tutorial, you will write a doorbell program and progressively
enhance it:

- Write the control logic in **Structured Text**, the programming language
  defined by IEC 61131-3.
- **Run** it directly from the editor and inspect the results.
- **Configure** the application with a task schedule.
- **Connect** it to hardware inputs and outputs.

Let's start writing code.

Continue to :doc:`helloworld`.
