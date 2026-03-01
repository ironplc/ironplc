=======================
Structured Text Basics
=======================

Structured Text (ST) is the primary language supported by IronPLC. This page
gives you a conceptual overview of the language so you know what to expect
when you start writing code. For hands-on practice, follow the
:doc:`/quickstart/index`.

--------------------------------------
What Structured Text Looks Like
--------------------------------------

If you have used Pascal, Ada, or even a bit of Python, Structured Text will
feel familiar. Here is a small example:

.. code-block::

   PROGRAM main
      VAR
         Counter : INT := 0;
         Limit : INT := 100;
         Running : BOOL := TRUE;
      END_VAR

      IF Running AND (Counter < Limit) THEN
         Counter := Counter + 1;
      ELSE
         Running := FALSE;
      END_IF;

   END_PROGRAM

Some things to notice:

- Keywords like :code:`PROGRAM`, :code:`VAR`, :code:`IF`, and :code:`THEN`
  are uppercase by convention.
- Variable declarations go inside a :code:`VAR` ... :code:`END_VAR` block
  at the top.
- Assignment uses ``:=`` (not ``=``).
- Statements end with a semicolon.
- Blocks are closed with :code:`END_PROGRAM`, :code:`END_IF`, and so on
  rather than curly braces.

--------------------------------------
Data Types
--------------------------------------

IEC 61131-3 provides a set of elementary data types:

.. list-table::
   :header-rows: 1
   :widths: 20 30 50

   * - Type
     - Size
     - Description
   * - :code:`BOOL`
     - 1 bit
     - Boolean (TRUE or FALSE)
   * - :code:`INT`
     - 16 bits
     - Signed integer (-32768 to 32767)
   * - :code:`DINT`
     - 32 bits
     - Double integer
   * - :code:`REAL`
     - 32 bits
     - Floating-point number
   * - :code:`STRING`
     - Variable
     - Character string
   * - :code:`TIME`
     - —
     - Duration (for example, ``T#100ms``)

You can also define your own types: enumerations, arrays, structures, and
subranges. These are covered in the :doc:`/reference/language/data-types/index`.

--------------------------------------
Control Flow
--------------------------------------

Structured Text supports the control flow statements you would expect:

**IF / ELSIF / ELSE:**

.. code-block::

   IF Temperature > 100.0 THEN
      Alarm := TRUE;
   ELSIF Temperature > 80.0 THEN
      Warning := TRUE;
   ELSE
      Alarm := FALSE;
      Warning := FALSE;
   END_IF;

**CASE:**

.. code-block::

   CASE State OF
      0: Motor := FALSE;
      1: Motor := TRUE;
      2: Motor := FALSE;
         Alarm := TRUE;
   END_CASE;

**FOR:**

.. code-block::

   FOR i := 0 TO 9 DO
      Values[i] := 0;
   END_FOR;

**WHILE:**

.. code-block::

   WHILE Buffer <> 0 DO
      Count := Count + 1;
      Buffer := Buffer / 2;
   END_WHILE;

-----------------------------------------------
How ST Differs from General-Purpose Languages
-----------------------------------------------

Structured Text was designed for real-time control, which leads to some
differences compared to languages like Python or C:

- **No dynamic memory allocation.** All variables are declared at the top
  of a block and exist for the lifetime of the program. There is no
  ``malloc``, ``new``, or garbage collector.
- **No recursion.** Function calls cannot recurse. This keeps execution
  time predictable.
- **Cyclic execution.** Your program runs repeatedly as part of the scan
  cycle (see :doc:`what-is-iec-61131-3`). You do not write a ``main``
  function that runs once — you write logic that runs every scan.
- **Time literals.** Durations are first-class values: ``T#100ms``,
  ``T#2s``, ``T#1h30m``.

These constraints exist because PLCs must respond within strict time
deadlines. Predictability matters more than flexibility.

--------------------------------------
Next Steps
--------------------------------------

Now that you have a feel for the language, try writing your first program
in the :doc:`/quickstart/index`. When you need precise details about
syntax and semantics, consult the :doc:`/reference/compiler/index`.
