=============
Hello, World!
=============

Now that you've installed IronPLC, it's time to write a first program.

If you've ever learned another programming language, you likely started
by writing a "Hello, World" program to display that text.
Don't worry if you haven't learned another programming language
and haven't written a "Hello, World" program. IEC 61131-3 is different.

IEC 61131-3 is specifically designed for real-time automation controllers
that often do not have a display. In other words, there is
often no place to display "Hello, World". Other options, such as creating
a file are also unusual.

A "Hello, World" program in IEC 61131-3 is quite a bit different.

-------------------------------
The Sense-Control-Actuate Cycle
-------------------------------

Controllers normally operate as part of a sense-control-actuate cycle.
We'll start with a simple example to illustrate the idea: a door bell system.

Our door bell system contains a button (the sensor) and a buzzer (the actuator).
We desire that the buzzer makes noise when the button is pressed.
To do that, we use a controller to check the button state and if pressed
then enable the buzzer.

.. note::

   It is possible to design a simpler door bell system. This is a more complex
   example designed to illustrate how to use IEC 61131-3.

-------------------------------------
Create a Program with Structured Text
-------------------------------------

Run Visual Studio Code, then in Visual Studio Code:

#. In the main menu, select :menuselection:`File --> New File...`.
#. In the :guilabel:`New File...` dialog, select the :menuselection:`Structured Text File` option.
#. Enter the code in :ref:`Hello World <helloworld>` into the :guilabel:`Editor`.

   .. code-block::
      :caption: Hello World
      :name: helloworld

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

#. Save the file with the name :file:`main.st`.

-----------------------------------
Anatomy of the Hello, World Program
-----------------------------------

Let's review this program. IEC 61131-3 applications are structured from blocks
called Program Organization Units (POUs). The :code:`PROGRAM` is a top level block and
similar to the "main" function in other languages. The piece indicated by

.. code-block::
   :name: main

   PROGRAM main

   END_PROGRAM

defines a :code:`PROGRAM` identified by the name :code:`main`.

Unlike the "main" function in other languages, a program does not run by default.
We need to tell the PLC runtime how we want to run the program. The piece indicated by

.. code-block::
   :name: config

   CONFIGURATION config
      
   END_CONFIGURATION

defines how we want the program to run. The configuration declares we want to execute
the :code:`main` program once every 100 ms. as the highest priority task. This task 
executed on the hardware element named :code:`res`.
