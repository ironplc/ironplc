==============================
Configuring Your Application
==============================

In the previous chapter, you compiled and ran a doorbell program. But it
only ran once. On a real PLC, the sense-control-actuate cycle repeats
continuously on a fixed schedule. In this chapter, you will add a
configuration to schedule the program and a timer to make the buzzer
pulse automatically.

--------------------------------------
Add a Timer
--------------------------------------

A doorbell that buzzes constantly is not very useful. Let's add a timer
so the buzzer pulses — turning on for a short duration, then turning off.

Open :file:`main.st` and replace its contents with:

.. code-block::
   :caption: main.st — Doorbell with Timer
   :name: doorbell-timer

   PROGRAM main
      VAR
         Button : BOOL;
         Buzzer : BOOL;
         PulseTimer : TON;
      END_VAR

      PulseTimer(IN := NOT Button, PT := T#500ms);
      Buzzer := PulseTimer.Q;

   END_PROGRAM

--------------------------------------
What Changed
--------------------------------------

We added a **timer on delay** (``TON``) function block:

- ``PulseTimer : TON`` declares an instance of the standard ``TON``
  function block. ``TON`` turns its output ``Q`` to ``TRUE`` after a
  specified delay.

- ``PulseTimer(IN := NOT Button, PT := T#500ms)`` calls the timer.
  ``IN`` is the enable input (``TRUE`` when the button is not pressed),
  and ``PT`` is the delay duration (500 milliseconds).

- ``Buzzer := PulseTimer.Q`` reads the timer's output. ``Q`` becomes
  ``TRUE`` once the timer has been running for 500 ms.

--------------------------------------
Add a Configuration Block
--------------------------------------

Now add a **configuration** to tell the runtime how to schedule the
program. Add the following **below** the existing :code:`END_PROGRAM`:

.. code-block::
   :caption: main.st — Complete Application
   :name: complete-app

   PROGRAM main
      VAR
         Button : BOOL;
         Buzzer : BOOL;
         PulseTimer : TON;
      END_VAR

      PulseTimer(IN := NOT Button, PT := T#500ms);
      Buzzer := PulseTimer.Q;

   END_PROGRAM

   CONFIGURATION config
      RESOURCE res ON PLC
         TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
         PROGRAM plc_task_instance WITH plc_task : main;
      END_RESOURCE
   END_CONFIGURATION

--------------------------------------
What the Configuration Does
--------------------------------------

The configuration block introduces three layers:

**CONFIGURATION**
   The top-level container for your application. Every IEC 61131-3
   application has exactly one configuration. Here it is named ``config``.

**RESOURCE**
   Represents a processing unit (a CPU or core). The resource named ``res``
   runs ``ON PLC``, where ``PLC`` is the name of the hardware target
   defined by the runtime environment.

**TASK**
   Defines a scheduling policy. The task named ``plc_task`` runs every
   100 milliseconds at priority level 1. This means the runtime will
   execute the sense-control-actuate cycle 10 times per second.

The line:

.. code-block::

   PROGRAM plc_task_instance WITH plc_task : main;

creates an **instance** of the ``main`` program, names it
``plc_task_instance``, and binds it to ``plc_task``. Every 100 ms, the
runtime reads the inputs, runs ``main``, and writes the outputs.

.. tip::

   For a deeper look at how these layers fit together, see
   :doc:`/explanation/program-organization`.

--------------------------------------
Run the Updated Program
--------------------------------------

Now that the timer and configuration are in place, run the updated program:

#. Click :guilabel:`Run Program` above the ``PROGRAM main`` line.
#. The :guilabel:`IronPLC Run` output panel shows the variables updating
   in real time. After a moment, you should see output like:

   .. code-block:: text

      Scan cycle: 10
      ---
        Button : BOOL = FALSE
        Buzzer : BOOL = TRUE
        PulseTimer.IN : BOOL = TRUE
        PulseTimer.PT : TIME = T#500ms
        PulseTimer.Q : BOOL = TRUE
        PulseTimer.ET : TIME = T#1000ms

#. Click :guilabel:`Stop` above the ``PROGRAM`` line to end execution.

The timer's elapsed time (``ET``) shows how long it has been running.
After enough scan cycles, ``Q`` becomes ``TRUE`` and the buzzer turns on.

--------------------------------------
Next Steps
--------------------------------------

Your :file:`main.st` file now contains everything needed for a working
IEC 61131-3 application. As your project grows, you will want to organize
code across multiple files.

Continue to :doc:`multiple-files`.
