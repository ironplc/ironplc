.. meta::
   :description: Find a bug in an IEC 61131-3 program with the IronPLC debugger: set a breakpoint, step through Structured Text, and watch variables change across scan cycles.

======================
Debugging Your Program
======================

.. include:: /includes/debugging-in-development.rst

Your doorbell works. In this chapter you add a feature to it, the feature
goes wrong, and you use the debugger to find out why.

That is what the debugger is for: not touring buttons, but seeing what your
program actually does between one scan and the next.

--------------------------------------
Add a Ring Counter
--------------------------------------

Suppose you want to know how many times the doorbell has rung. Open
:file:`main.st` and add a counter:

.. code-block::
   :caption: main.st --- doorbell with a ring counter

   PROGRAM main
      VAR
         Button : BOOL;
         Buzzer : BOOL;
         RingCount : DINT;
         PulseTimer : TON;
      END_VAR

      PulseTimer(IN := NOT Button, PT := T#500ms);
      Buzzer := PulseTimer.Q;

      IF Buzzer THEN
         RingCount := RingCount + 1;
      END_IF;

   END_PROGRAM

   CONFIGURATION config
      RESOURCE res ON PLC
         TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
         PROGRAM plc_task_instance WITH plc_task : main;
      END_RESOURCE
   END_CONFIGURATION

The logic reads correctly in English: when the buzzer sounds, add one to the
count.

Click :guilabel:`Run Program` and let it run for a few seconds, as you did in
the last chapter. ``RingCount`` is not 1. It is in the hundreds of thousands,
and still climbing.

--------------------------------------
Set Up the Debugger
--------------------------------------

Rather than guess, stop the program on the line that is wrong and watch it
work.

#. Click the gutter to the left of the line ``RingCount := RingCount + 1;``.
   A red dot appears.

#. Open the :guilabel:`Run and Debug` view and select
   :guilabel:`create a launch.json file`. Choose :guilabel:`IronPLC` when
   prompted, then set ``stopOnEntry`` to ``true``:

   .. code-block:: json
      :caption: .vscode/launch.json

      {
        "version": "0.2.0",
        "configurations": [
          {
            "type": "ironplc",
            "request": "launch",
            "name": "IronPLC: Debug Active File",
            "program": "${file}",
            "stopOnEntry": true
          }
        ]
      }

``stopOnEntry`` pauses the program before it does anything, so you can watch
from the very first scan.

#. Press :kbd:`F5`.

The program compiles, starts, and pauses immediately on the first line of the
program body. Nothing has run yet.

--------------------------------------
Read the Variables
--------------------------------------

Open the :guilabel:`Variables` view. It shows two groups.

:guilabel:`Program` holds the variables you declared:

.. code-block:: text

   Button     BOOL    FALSE
   Buzzer     BOOL    FALSE
   RingCount  DINT    0
   PulseTimer TON     0

:guilabel:`Runtime` holds one value the virtual machine owns:

.. code-block:: text

   scanCount  ULINT   0

``scanCount`` is the number of scan cycles the program has completed. It is
0, because none have. This is the number to watch.

--------------------------------------
Run to the Breakpoint
--------------------------------------

Press :kbd:`F5` to continue.

The program runs, and stops on the line you marked:

.. code-block:: text

   Buzzer     BOOL    TRUE
   RingCount  DINT    0
   scanCount  ULINT   500

Two things to notice.

``scanCount`` is 500. The program ran 500 complete scan cycles before the
buzzer turned on, because ``PulseTimer`` waits ``T#500ms`` and the debugger
advances the clock by one millisecond per scan. Under the debugger, timers
are counted in scans, so a run is repeatable.

``RingCount`` is still 0. A breakpoint pauses *before* the line runs, so you
are looking at the values as they are on the way in.

--------------------------------------
Continue, and Watch
--------------------------------------

Press :kbd:`F5` again.

The program does not run to the end --- there is no end. It finishes the
scan, starts the next one, and stops on the same line again, because a
breakpoint in a PLC program pauses on **every** scan that reaches it:

.. code-block:: text

   Buzzer     BOOL    TRUE
   RingCount  DINT    1
   scanCount  ULINT   501

Press :kbd:`F5` twice more:

.. code-block:: text

   Buzzer     BOOL    TRUE
   RingCount  DINT    3
   scanCount  ULINT   503

There it is. ``RingCount`` rises by one every scan, and ``Buzzer`` was
already ``TRUE`` on every one of those scans. The doorbell rang once. The
program counted the scans during which it was ringing.

--------------------------------------
Fix the Bug
--------------------------------------

``IF Buzzer THEN`` asks "is the buzzer on?" --- and it is on for many scans in
a row. What you meant to ask is "has the buzzer *just turned* on?"

To answer that, the program has to remember what the buzzer was doing on the
previous scan. Stop the session with :kbd:`Shift+F5` and change the program:

.. code-block::
   :caption: main.st --- counting the rising edge

   PROGRAM main
      VAR
         Button : BOOL;
         Buzzer : BOOL;
         RingCount : DINT;
         PrevBuzzer : BOOL;
         PulseTimer : TON;
      END_VAR

      PulseTimer(IN := NOT Button, PT := T#500ms);
      Buzzer := PulseTimer.Q;

      IF Buzzer AND NOT PrevBuzzer THEN
         RingCount := RingCount + 1;
      END_IF;
      PrevBuzzer := Buzzer;

   END_PROGRAM

``PrevBuzzer`` holds what ``Buzzer`` was on the previous scan. The count now
changes only on the scan where the buzzer turns on --- the **rising edge**.

Press :kbd:`F5`, then :kbd:`F5` again to leave the entry pause. The program
stops on the counter line once, at ``scanCount`` 500, exactly as before.

Press :kbd:`F5` again. Nothing stops. The program keeps scanning, the
condition is never true again, and ``RingCount`` stays where it is.

--------------------------------------
Next Steps
--------------------------------------

You can now stop a running program and watch it work a scan at a time. As
your project grows, you will want to organize code across multiple files.

Continue to :doc:`multiple-files`.
