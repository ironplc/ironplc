==============================
Configuring Your Application
==============================

In the previous chapter, you wrote a program with inputs and outputs. But
a program alone is not a complete IEC 61131-3 application — you need to
tell the runtime *how often* to run it and *on what hardware*. That is the
job of a configuration.

--------------------------------------
Add a Configuration Block
--------------------------------------

Open :file:`main.st` and add the following **below** the existing
:code:`END_PROGRAM`:

.. code-block::
   :caption: Complete Application
   :name: complete-app

   PROGRAM main
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
   END_CONFIGURATION

Save the file. The IronPLC extension should show no errors.

--------------------------------------
What the Configuration Does
--------------------------------------

The new block introduces three layers:

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
You Now Have a Complete Application
--------------------------------------

Your :file:`main.st` file now contains everything needed for a deployable
IEC 61131-3 application:

- A **program** that reads a button and controls a buzzer.
- A **configuration** that runs the program every 100 ms.

In the next chapter, you will learn how to split this into multiple files
as your project grows.

Continue to :doc:`multiple-files`.
