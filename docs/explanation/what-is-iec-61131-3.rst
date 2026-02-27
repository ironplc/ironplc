======================
What is IEC 61131-3?
======================

If you are new to industrial automation, this page gives you the background
you need before writing your first program.

-------------------------------
Programmable Logic Controllers
-------------------------------

Factories, power plants, water treatment facilities, and countless other
systems rely on small, dedicated computers called **Programmable Logic
Controllers** (PLCs). A PLC reads inputs from sensors (buttons, temperature
probes, limit switches), runs a control program, and writes outputs to
actuators (motors, valves, indicator lights).

PLCs have been around since the late 1960s. Over time every manufacturer
developed its own programming language, which made it hard to move programs
between vendors or train engineers on more than one platform.

--------------------------------------
The IEC 61131-3 Standard
--------------------------------------

**IEC 61131-3** is the international standard that defines programming
languages for PLCs. Published by the International Electrotechnical
Commission (IEC), it provides a common set of languages and rules so that
engineers can write programs that are portable across different hardware.

The standard defines five programming languages:

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Language
     - Description
   * - **Structured Text (ST)**
     - A high-level textual language that resembles Pascal. This is the
       primary language supported by IronPLC.
   * - **Ladder Diagram (LD)**
     - A graphical language based on electrical relay logic diagrams.
   * - **Function Block Diagram (FBD)**
     - A graphical language that wires together reusable function blocks.
   * - **Instruction List (IL)**
     - A low-level textual language similar to assembly. Deprecated in the
       third edition of the standard.
   * - **Sequential Function Chart (SFC)**
     - A graphical language for describing sequential processes with steps
       and transitions. IronPLC has partial support for SFC.

Most working engineers use a mix of these languages. IronPLC focuses on
Structured Text because it is the most expressive and the easiest to work
with using standard software development tools like text editors and version
control.

--------------------------------------
The Scan Cycle
--------------------------------------

A PLC does not run a program once and exit the way a desktop application
does. Instead, it executes a **scan cycle** that repeats continuously:

1. **Read inputs** — sample all sensors and store the values in memory.
2. **Execute program** — run the control logic using the input values.
3. **Write outputs** — send the computed results to actuators.
4. **Repeat** — go back to step 1.

Each pass through the cycle is called a **scan**. A typical scan takes
between 1 and 100 milliseconds depending on the complexity of the program
and the speed of the hardware. This is what makes PLC programs
**real-time**: they guarantee that inputs are read and outputs are
updated within a predictable time.

This cycle is fundamentally different from event-driven programming (like
a web server waiting for HTTP requests) or batch processing (like a script
that runs once). When you write IEC 61131-3 code, you are writing the logic
for a single scan, and the runtime takes care of calling it repeatedly.

--------------------------------------
Why Does This Matter for IronPLC?
--------------------------------------

Understanding the scan cycle helps you make sense of concepts you will
encounter in the tutorials:

- **Tasks** define *how often* a scan runs (for example, every 100 ms).
- **Programs** contain the logic that runs on *each scan*.
- **Variables** hold state that persists *across scans*.
- **Directly represented variables** map to *physical inputs and outputs*
  that the scan cycle reads and writes.

These ideas are explored in more detail in :doc:`program-organization`
and :doc:`variables-and-io`.
