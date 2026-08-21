========================
Debugging a Scan Cycle
========================

.. include:: /includes/debugging-in-development.rst

Debuggers were built for programs that start, do something, and stop. A PLC
program does none of those things. It runs the same code again and again,
forever, and it does so while a physical machine responds to its outputs.

That difference explains almost everything about how the IronPLC debugger
behaves --- including the things it deliberately will not do.

A Breakpoint Fires Every Scan
=============================

In an ordinary program, a breakpoint on a line usually stops you once. In a
PLC program it stops you on every scan cycle that reaches that line. A task
running every 100 ms reaches it ten times a second.

So :guilabel:`Continue` does not mean "run to the end". There is no end. It
means "finish this scan, start the next one, and stop here again".

This is why the :guilabel:`Runtime` scope exposes ``scanCount``. Without it,
two consecutive stops on the same line look identical, and you cannot tell
whether a value changed because your logic changed it or because you are
looking at a different cycle. With it, you can watch a value accumulate
across cycles --- which is how most PLC bugs actually reveal themselves.

See :doc:`/quickstart/debugging` for a worked example.

Pausing Stops the Program, Not the World
========================================

When the debugger pauses at a breakpoint, your program stops. Nothing else
does.

On a desktop this is harmless. On a machine it is the whole problem: a
conveyor keeps moving, a tank keeps filling, a heater keeps heating. The
inputs your program will read when you press :guilabel:`Continue` describe a
world that moved on while you were reading the variables pane, and the outputs
it wrote are still applied.

IronPLC debugs on your computer rather than on a machine, so today this is a
question of interpretation rather than safety. What you are looking at is a
snapshot of one cycle, taken out of a sequence that was meant to run
uninterrupted.

Time Advances by the Scan, Not by the Clock
===========================================

A timer raises an obvious problem for a debugger. If a ``TON`` measures real
elapsed time, then how long you spend reading the variables pane changes what
your program does --- and a session you cannot repeat is a session you cannot
learn from.

Under the debugger, program time does not come from the wall clock. It
advances by one millisecond for every scan cycle the program completes. A
``T#500ms`` timer therefore elapses on scan 500, on every machine, every time
you run it, no matter how long you spent paused.

Two things follow. Debug sessions are repeatable: the same program stops at
the same breakpoint with the same values. And a timer under the debugger
measures scans rather than seconds --- which is why ``scanCount`` and a
timer's delay can be compared directly.

:program:`ironplcvm` behaves differently: outside the debugger, timers use the
real clock, as a PLC must.

Why You Cannot Change a Value
=============================

Most debuggers let you set a variable while stopped. IronPLC does not, and
this is the limitation people ask about first.

The reason is that a PLC already has a feature with that name, and it does
something stronger. In industrial practice, *forcing* a variable means
holding it at a value **across scans**, overriding whatever the program
writes, until you release it. It is a commissioning tool: you force an output
to test a valve without running the machine that would normally command it.

A simple "set this variable while paused" looks like forcing and is not. The
value survives exactly until the next line of your program assigns to it ---
which, in a scan cycle, is usually a few milliseconds later. You would set a
value, press :guilabel:`Continue`, watch it vanish, and reasonably conclude
the debugger was broken.

So the choice was between a real force table, with the semantics PLC
engineers already expect, and nothing. IronPLC does nothing for now. To
change what the program does, change the program and launch it again.

Why the Debugger Cannot Interrupt a Running Program
===================================================

:guilabel:`Pause` is not available. You stop the program by setting a
breakpoint before you launch, or by limiting the run with ``scanLimit``.

The debug server runs on a single thread. It executes your program, and it
reads the next request from the editor when the program stops --- at a
breakpoint, a step landing, entry, a trap, or completion. Nothing is
listening while a scan is in flight, so there is nothing to receive a pause.

The alternative --- a second thread watching for requests and setting a flag
the virtual machine checks between instructions --- is how a mature debugger
does it, and it is planned. It was not the first thing to build, because the
common case is knowing where you want to stop before you start.

Why One Program Instance
========================

The debugger refuses to launch a configuration that declares more than one
program instance.

Consider a breakpoint inside a POU that two instances both execute. The first
instance to reach it stops the whole virtual machine. The variables you
inspect belong to that instance. The other instance has not run yet this
scan, so its state is left over from the previous cycle --- and nothing on
screen tells you which of those two things you are looking at.

Answering that properly means teaching the debugger to name instances and let
you choose between them. Until then, refusing the launch is a clearer answer
than a variables pane that quietly mixes two cycles together.

Where Debug Information Lives
=============================

Source-level debugging needs a translation between the bytecode the virtual
machine executes and the Structured Text you wrote. That translation is the
**debug section** of the compiled container: a map from bytecode offsets to
source lines, the names and types of your variables, the names of your POUs,
and a table of the source files the program was compiled from.

Three consequences follow.

**Debugging costs nothing when you are not debugging.** The debug section is
data at the end of the container, separate from the code. The bytecode the
virtual machine executes is the same either way, and carrying the debug
section does not slow a program down. There is no separate debug build, and no
reason to have one --- which is why :program:`ironplcc compile` simply always
writes it. Other tools read it too: ``ironplcvm run --dump-vars`` uses it to
report variables by name.

**Breakpoints work in a container built from many files.** Each line map entry
records which source file it came from, so a breakpoint you set in one file
binds to code generated from that file.

**The container remembers what it was compiled from.** The source file table
records a hash of each file. That is what makes it possible to tell that the
container you are debugging was built from a different version of the source
than the one on screen.

See Also
========

* :doc:`execution-cycle` --- how the scan cycle works
* :doc:`/quickstart/debugging` --- find a scan-cycle bug with the debugger
* :doc:`/reference/editor/debugging` --- what the debugger supports today
