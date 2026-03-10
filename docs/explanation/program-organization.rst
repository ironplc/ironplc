====================
Program Organization
====================

IEC 61131-3 programs are organized in layers. This page explains what
each layer does and why the structure exists. For hands-on practice building
up these layers step by step, follow the :doc:`/quickstart/index`.

--------------------------------------
The Big Picture
--------------------------------------

An IEC 61131-3 application is built from these building blocks, from
innermost to outermost:

.. code-block:: text

   CONFIGURATION
   └── RESOURCE
       └── TASK
           └── PROGRAM instance
               └── Your control logic

Each layer has a specific purpose:

- **PROGRAM** contains the control logic (the code you write).
- **TASK** defines *how often* a program runs.
- **RESOURCE** represents a processing unit (a CPU or core).
- **CONFIGURATION** ties everything together for a specific hardware setup.

This layered model separates *what the program does* from *how and where it
runs*. You can change the scan rate or move a program to different hardware
without rewriting the logic.

--------------------------------------
Programs
--------------------------------------

A :code:`PROGRAM` is the basic unit of control logic. It declares variables
and contains statements that execute on every scan:

.. code-block::

   PROGRAM main
      VAR
         Counter : INT := 0;
      END_VAR

      Counter := Counter + 1;

   END_PROGRAM

Programs are similar to classes in object-oriented languages: they bundle
data (variables) with behavior (statements). Unlike classes, programs are
not instantiated with :code:`new` — they are instantiated by a
:code:`CONFIGURATION` (see below).

Variables inside a program retain their values between scans. The
:code:`Counter` variable above will be 1 after the first scan, 2 after the
second, and so on.

--------------------------------------
Functions and Function Blocks
--------------------------------------

In addition to programs, IEC 61131-3 defines two other kinds of
**Program Organization Units** (POUs):

- **Functions** are stateless: they take inputs, compute a result, and
  return it. They do not retain values between calls. Think of them like
  pure functions.
- **Function Blocks** are stateful: like programs, they have internal
  variables that persist between calls. Think of them like objects that
  you can instantiate multiple times.

You use functions and function blocks inside programs to organize your
logic into reusable pieces.

--------------------------------------
Tasks
--------------------------------------

A :code:`TASK` defines a scheduling policy. The most common kind is a
periodic task that runs at a fixed interval:

.. code-block::

   TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);

This says: "run every 100 milliseconds, at priority level 1."

Tasks control the scan rate. A fast task (``T#10ms``) reads inputs and
writes outputs more frequently, which gives tighter control but uses more
CPU. A slow task (``T#1s``) is gentler on the CPU but less responsive.

You can define multiple tasks with different intervals — for example, a
fast task for motion control and a slow task for temperature monitoring.

--------------------------------------
Resources
--------------------------------------

A :code:`RESOURCE` represents a processing unit — typically a CPU or
core. It groups tasks and program instances that run on the same hardware:

.. code-block::

   RESOURCE res ON PLC
      TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
      PROGRAM plc_task_instance WITH plc_task : main;
   END_RESOURCE

The line ``PROGRAM plc_task_instance WITH plc_task : main`` means: "create
an instance of the :code:`main` program, name it :code:`plc_task_instance`,
and run it on :code:`plc_task`."

The ``ON PLC`` part names the hardware target. In practice, the target
name is defined by the runtime environment.

--------------------------------------
Configurations
--------------------------------------

A :code:`CONFIGURATION` is the top-level container. It holds one or more
resources and represents a complete deployable unit:

.. code-block::

   CONFIGURATION config
      RESOURCE res ON PLC
         TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
         PROGRAM plc_task_instance WITH plc_task : main;
      END_RESOURCE
   END_CONFIGURATION

Every IEC 61131-3 application needs exactly one configuration.

--------------------------------------
Putting It All Together
--------------------------------------

Here is a complete, minimal application showing all the layers:

.. code-block::

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

Reading from the bottom up:

1. The **configuration** named ``config`` defines one resource.
2. The **resource** named ``res`` defines one task and one program instance.
3. The **task** named ``plc_task`` runs every 100 ms.
4. The **program** named ``main`` reads a button input and controls a buzzer.

For a step-by-step guide to building this up from scratch, see the
:doc:`/quickstart/index`.
