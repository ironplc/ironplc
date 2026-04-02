.. meta::
   :description: Type conversion rules in IronPLC, including implicit integer widening and explicit conversion functions.

================
Type Conversions
================

IronPLC follows the IEC 61131-3 type hierarchy for conversions between
elementary data types. Some conversions happen implicitly (the compiler inserts
them automatically); others require an explicit conversion function call.

----------------------------
Implicit Integer Widening
----------------------------

A value of a narrower integer type can be passed wherever a wider integer type
is expected. This applies to function arguments, function return values
assigned to variables, and bare integer literals.

The rule is: an implicit conversion is allowed when the source type's full
value range fits inside the target type's value range.

.. list-table:: Implicit integer widening matrix
   :header-rows: 1
   :stub-columns: 1

   * -
     - SINT
     - INT
     - DINT
     - LINT
     - USINT
     - UINT
     - UDINT
     - ULINT
   * - **SINT**
     - =
     - Yes
     - Yes
     - Yes
     - No
     - No
     - No
     - No
   * - **INT**
     - No
     - =
     - Yes
     - Yes
     - No
     - No
     - No
     - No
   * - **DINT**
     - No
     - No
     - =
     - Yes
     - No
     - No
     - No
     - No
   * - **LINT**
     - No
     - No
     - No
     - =
     - No
     - No
     - No
     - No
   * - **USINT**
     - No
     - Yes
     - Yes
     - Yes
     - =
     - Yes
     - Yes
     - Yes
   * - **UINT**
     - No
     - No
     - Yes
     - Yes
     - No
     - =
     - Yes
     - Yes
   * - **UDINT**
     - No
     - No
     - No
     - Yes
     - No
     - No
     - =
     - Yes
   * - **ULINT**
     - No
     - No
     - No
     - No
     - No
     - No
     - No
     - =

Read each row as "source type" and each column as "target type". **Yes** means
the compiler accepts the conversion implicitly. **No** means an explicit
conversion function is required.

Widening chains
~~~~~~~~~~~~~~~

- **Signed:** ``SINT`` |rarr| ``INT`` |rarr| ``DINT`` |rarr| ``LINT``
- **Unsigned:** ``USINT`` |rarr| ``UINT`` |rarr| ``UDINT`` |rarr| ``ULINT``
- **Cross-sign:** an unsigned type can widen to a signed type of strictly
  greater bit width (e.g. ``USINT`` |rarr| ``INT``, ``UINT`` |rarr| ``DINT``).

.. |rarr| unicode:: U+2192

Example
~~~~~~~

.. code-block::

   FUNCTION TAKES_DINT : BOOL
   VAR_INPUT
       in : DINT;
   END_VAR
       TAKES_DINT := in > 0;
   END_FUNCTION

   PROGRAM main
   VAR
       i : INT := 5;
       r : BOOL;
   END_VAR
       (* Accepted: INT widens to DINT *)
       r := TAKES_DINT(in := i);
   END_PROGRAM

----------------------------
Narrowing Conversions
----------------------------

Converting from a wider type to a narrower type (e.g. ``DINT`` to ``INT``) can
lose data, so IronPLC requires an explicit conversion function:

.. code-block::

   VAR
       big : DINT := 100000;
       small : INT;
   END_VAR
       small := DINT_TO_INT(big);  (* explicit narrowing *)

----------------------------
Bit-String Types
----------------------------

Bit-string types (``BYTE``, ``WORD``, ``DWORD``, ``LWORD``) are a separate
type family. There is no implicit conversion between bit-string types and
integer types. Use explicit conversion functions such as ``BYTE_TO_INT`` or
``INT_TO_WORD``.

----------------------------
Integer and Real Types
----------------------------

There is no implicit conversion between integer types and real types
(``REAL``, ``LREAL``). Use explicit conversion functions such as
``INT_TO_REAL`` or ``REAL_TO_INT``.

Bare (untyped) integer literals like ``0`` or ``42`` are an exception: they
can be passed to ``REAL`` or ``LREAL`` parameters because the literal has no
declared type and the compiler infers the target type.

-----------------------------
Explicit Conversion Functions
-----------------------------

IEC 61131-3 defines conversion functions for all type pairs. The naming
convention is ``<SOURCE>_TO_<TARGET>``:

.. code-block::

   INT_TO_REAL(x)      (* integer to real *)
   DINT_TO_INT(x)      (* narrowing: DINT to INT *)
   BYTE_TO_INT(x)      (* bit-string to integer *)
   REAL_TO_INT(x)      (* real to integer, truncates *)
