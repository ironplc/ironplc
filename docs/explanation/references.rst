============================
References and Indirection
============================

This page explains what references are, why they exist in IEC 61131-3, and
when to reach for them in your programs. For the complete syntax and operator
list, see the :doc:`/reference/language/data-types/reference-types`.

.. note::

   References require enabling through a dialect or flag. You can select a
   dialect that includes references (``--dialect iec61131-3-ed3`` or
   ``--dialect rusty``) or enable references directly with ``--allow-ref-to``.
   See :doc:`enabling-features` for details.

--------------------------------------
What Is a Reference?
--------------------------------------

Most variables hold a value directly — an ``INT`` contains a number, a
``BOOL`` contains ``TRUE`` or ``FALSE``. A **reference** is different: instead
of holding a value, it holds the identity of another variable. You can think
of it as a label that says *"look at that variable over there."*

.. code-block::

   VAR
     counter : INT := 42;
     r : REF_TO INT := REF(counter);
   END_VAR

   r^ := 10;   (* changes counter to 10 *)

After the assignment through ``r^``, the variable ``counter`` itself is 10.
The reference ``r`` and the variable ``counter`` are two ways to reach the
same data.

--------------------------------------
Creating and Using References
--------------------------------------

Working with references involves three pieces:

**Declaring** — use ``REF_TO`` to say that a variable will hold a reference:

.. code-block::

   r : REF_TO INT;

**Creating** — use ``REF(variable)`` to make a reference that points to an
existing variable:

.. code-block::

   r := REF(counter);

**Dereferencing** — use ``^`` to follow the reference and access the original
variable, for both reading and writing:

.. code-block::

   value := r^;    (* read counter through the reference *)
   r^ := 99;       (* write 99 into counter *)

You can also create named reference types with ``TYPE``:

.. code-block::

   TYPE
     IntRef : REF_TO INT;
   END_TYPE

--------------------------------------
Null References
--------------------------------------

A reference does not always have a target. The literal ``NULL`` represents an
empty reference — one that points to nothing.

.. code-block::

   r := NULL;

If you dereference ``NULL``, the runtime stops execution with a clear error
rather than reading meaningless data
(see :doc:`/reference/runtime/problems/V4004`). To avoid this, check before
dereferencing:

.. code-block::

   IF r <> NULL THEN
     value := r^;
   END_IF;

Uninitialized ``REF_TO`` variables default to ``NULL``, so you should always
either assign an initial value or check before first use.

--------------------------------------
Why References Are Safe in IronPLC
--------------------------------------

IronPLC references are designed with safety as a priority. The compiler and
runtime work together to enforce the following guarantees:

**No pointer arithmetic by default.**
References identify variables, not raw memory addresses. By default, you
cannot add, subtract, or use ordering comparisons on a reference. If you need
pointer arithmetic for compatibility with other PLC environments, you can
enable it with ``--allow-pointer-arithmetic``.

**Type safety.**
A ``REF_TO INT`` can only point to an ``INT``. Assigning it to a
``REF_TO REAL`` variable is a compile-time error.

**No dangling references.**
The compiler prevents you from taking a reference to a temporary variable
(``VAR_TEMP``, function-local parameters) that will be destroyed when the
call returns. This eliminates a common source of subtle bugs.

**No nested references.**
``REF_TO REF_TO`` is not allowed. One level of indirection is sufficient for
PLC applications, and limiting depth keeps programs easy to reason about.

**Runtime null check.**
Dereferencing ``NULL`` does not silently read garbage — it stops the program
with a diagnostic error so you can find and fix the bug.

These restrictions are deliberate. PLC programs often control physical
processes where a subtle memory error can cause real harm. Safety is more
valuable than flexibility here.

--------------------------------------
References vs VAR_IN_OUT
--------------------------------------

IEC 61131-3 already has a mechanism for passing data by reference:
``VAR_IN_OUT`` parameters (see :doc:`variables-and-io`). How do explicit
``REF_TO`` references differ?

.. list-table::
   :header-rows: 1
   :widths: 30 35 35

   * -
     - ``VAR_IN_OUT``
     - ``REF_TO``
   * - Set by
     - The caller, at invocation time
     - Your code, at any point during execution
   * - Can be reassigned
     - No — fixed for the duration of the call
     - Yes — can point to different variables over time
   * - Can be null
     - No — the caller must always provide a variable
     - Yes — can be ``NULL``
   * - Can be stored
     - No — only valid inside the called POU
     - Yes — can be kept in a function block instance

**Rule of thumb:** use ``VAR_IN_OUT`` for straightforward pass-by-reference
parameters. Use ``REF_TO`` when you need to change the target at runtime,
store a reference for later, or when ``NULL`` is a meaningful state.

--------------------------------------
When to Use References
--------------------------------------

References are most useful when a program needs to decide *which* variable
to work with at runtime. Here are common patterns:

**Selecting among alternatives.** A reference can point to one of several
variables based on a condition — for example, choosing which sensor to read
depending on a mode selector.

**Storing a reference for later.** A function block can accept a ``REF_TO``
variable and hold onto it across scan cycles, operating on the referenced
data each time it runs.

**Arrays of references.** You can declare ``ARRAY[0..N] OF REF_TO INT`` to
build a collection of variables to process in a loop, even when the variables
themselves are not in an array.

.. playground::
   :dialect: 2013

   PROGRAM main
     VAR
       sensorA : INT := 10;
       sensorB : INT := 20;
       mode : INT := 1;
       active : REF_TO INT;
       reading : INT;
     END_VAR

     (* Select which sensor to read based on mode *)
     IF mode = 1 THEN
       active := REF(sensorA);
     ELSE
       active := REF(sensorB);
     END_IF;

     IF active <> NULL THEN
       reading := active^;
     END_IF;
   END_PROGRAM

--------------------------------------
Restrictions
--------------------------------------

References in IronPLC have intentional limits that keep programs predictable:

- ``REF()`` only accepts simple named variables — not array elements, struct
  fields, or expressions. This ensures every reference points to a
  well-defined location in the variable table.
- By default, only ``=`` and ``<>`` comparisons are allowed on references.
  Arithmetic and ordering comparisons can be enabled with
  ``--allow-pointer-arithmetic``.

For the complete list of restrictions and related compiler diagnostics, see
:doc:`/reference/language/data-types/reference-types`.

--------------------------------------
Next Steps
--------------------------------------

- :doc:`/reference/language/data-types/reference-types` — full syntax,
  operators, and problem codes
- :doc:`enabling-features` — how to enable references via dialects or flags
- :doc:`/quickstart/index` — hands-on tutorials
