=============
Hello, World!
=============

Now that you've installed IronPLC, it's time to write a first program.

If you've ever learned another programming language, you likely started
by writing a "Hello, World" program to display that text.
Don't worry if you haven't learned another programming language
or haven't written a "Hello, World" program. 

IEC 61131-3 is designed for real-time automation controllers
that often do not have a display. In other words, there is
often no place to show "Hello, World". Other solutions to get feedback,
such as creating a file are also unusual.

In short, a "Hello, World" program in IEC 61131-3 is different.

-------------------------------
The Sense-Control-Actuate Cycle
-------------------------------

Controllers normally operate as part of a sense-control-actuate cycle.
We'll start with a simple example to illustrate the idea: a door bell system.
Our door bell system contains a button (the sensor) and a buzzer (the actuator).

.. figure:: button-buzzer.svg
   :width: 200
   
   Pressing the button triggers the buzzer.

We desire that the buzzer makes noise when the button is pressed.
To do that, we use a controller to check the button state and if pressed
then enable the buzzer.

.. note::

   It is possible to design a simpler door bell system. This
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
similar to the :code:`main` function in other languages. The section indicated by

.. code-block::
   :name: main

   PROGRAM main

   END_PROGRAM

defines a :code:`PROGRAM` having the name :code:`main`.

Unlike the :code:`main` function in other languages, a program does not run by default.
We need to tell the PLC runtime how we want to run the program. The piece indicated by

.. code-block::
   :name: config

   CONFIGURATION config
      
   END_CONFIGURATION

defines how we want the program to run. The configuration declares we want to execute
the :code:`main` program once every 100 ms and this task is the highest priority task. This task 
executed on the hardware element named :code:`res`.

We want our program to enable (or disable) the buzzer based on whether the button is
pressed. The piece indicated by

.. code-block:: 
   :name: var

   VAR

   END_VAR

defines two variables that will contain the state of the button and buzzer. We can then use
the variable containing the state of the button to control the variable containing the
desired state of the buzzer. The statement indicated by

.. code-block:: 
   :name: statement

   Buzzer := NOT Button;

does just that. In plain English, the statement says "assign the value of
:code:`Buzzer` to be the boolean inverse of the value of :code:`Button`."

From the perspective of the program, there is no specific meaning to the
names :code:`Buzzer` and :code:`Button`. We could have called them
:code:`foo` and :code:`bar`, but we choose names that were indicative of
their purpose.

Our program needs to associate the variables :code:`Buzzer` and :code:`Button`
with digital input/output. We do this by declaring these as directly represented
variables. Directly represented variable have specific physical or logical locations,
for example, being associated with a digital input pin. The declarations

.. code-block:: 
   :name: directly-represented

   AT %IX1

associates the variable :code:`Button` with a 1-bit (Boolean) input.

The net result of these elements is to define a program that every 100 ms, reads
from an input device, evaluates the logical inverse, and assign the result to
an output device. 