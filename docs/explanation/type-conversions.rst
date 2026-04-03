.. meta::
   :description: Type conversion rules in IronPLC, including implicit type widening for integers, reals, and bit-strings, and explicit conversion functions.

================
Type Conversions
================

IronPLC follows the IEC 61131-3 type hierarchy for conversions between
elementary data types. Some conversions happen implicitly (the compiler inserts
them automatically); others require an explicit conversion function call.

--------------------------
Implicit Type Widening
--------------------------

A value of a narrower type can be passed wherever a wider type is expected.
This applies to function arguments, function return values assigned to
variables, and bare integer literals.

The general rule is: an implicit conversion is allowed when the source type's
full value range is exactly representable in the target type.

IronPLC supports three categories of implicit widening:

1. **Integer widening** — within signed, unsigned, and cross-sign integer types
2. **Integer to real widening** — when the conversion is lossless
3. **Bit-string widening** — within the ``ANY_BIT`` family (``BOOL`` excluded)

Integer widening
~~~~~~~~~~~~~~~~

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

Integer to real widening
~~~~~~~~~~~~~~~~~~~~~~~~

An integer type can implicitly widen to a real type when every value of the
source type is exactly representable in the target type. ``REAL`` is a 32-bit
float with a 23-bit mantissa, so it covers integers up to 16 bits. ``LREAL``
is a 64-bit float with a 52-bit mantissa, so it covers all standard integer
types.

.. list-table:: Implicit integer to real widening matrix
   :header-rows: 1
   :stub-columns: 1

   * -
     - REAL
     - LREAL
   * - **SINT**
     - Yes
     - Yes
   * - **INT**
     - Yes
     - Yes
   * - **DINT**
     - No
     - Yes
   * - **LINT**
     - No
     - Yes
   * - **USINT**
     - Yes
     - Yes
   * - **UINT**
     - Yes
     - Yes
   * - **UDINT**
     - No
     - Yes
   * - **ULINT**
     - No
     - Yes

``DINT``, ``LINT``, ``UDINT``, and ``ULINT`` cannot implicitly widen to
``REAL`` because their value ranges exceed the 23-bit mantissa precision. Use
an explicit conversion such as ``DINT_TO_REAL(x)`` or widen to ``LREAL``
instead.

Real to integer is never implicit. Use explicit conversion functions such as
``REAL_TO_INT``.

Bit-string widening
~~~~~~~~~~~~~~~~~~~

Bit-string types widen by zero-extending to a wider container. ``BOOL`` is
excluded because it has boolean semantics (``TRUE``/``FALSE``), not
numeric bit-container semantics.

.. list-table:: Implicit bit-string widening matrix
   :header-rows: 1
   :stub-columns: 1

   * -
     - BYTE
     - WORD
     - DWORD
     - LWORD
   * - **BYTE**
     - =
     - Yes
     - Yes
     - Yes
   * - **WORD**
     - No
     - =
     - Yes
     - Yes
   * - **DWORD**
     - No
     - No
     - =
     - Yes
   * - **LWORD**
     - No
     - No
     - No
     - =

Widening chains
~~~~~~~~~~~~~~~

- **Signed:** ``SINT`` |rarr| ``INT`` |rarr| ``DINT`` |rarr| ``LINT``
- **Unsigned:** ``USINT`` |rarr| ``UINT`` |rarr| ``UDINT`` |rarr| ``ULINT``
- **Cross-sign:** an unsigned type can widen to a signed type of strictly
  greater bit width (e.g. ``USINT`` |rarr| ``INT``, ``UINT`` |rarr| ``DINT``).
- **Bit-string:** ``BYTE`` |rarr| ``WORD`` |rarr| ``DWORD`` |rarr| ``LWORD``
- **Integer to REAL (lossless):** ``SINT``, ``INT``, ``USINT``, ``UINT`` |rarr| ``REAL``
- **Integer to LREAL (lossless):** all integer types |rarr| ``LREAL``

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

Bit-string types (``BYTE``, ``WORD``, ``DWORD``, ``LWORD``) support implicit
widening within the bit-string family — see the bit-string widening matrix
above.

``BOOL`` is excluded from bit-string widening. Although IEC 61131-3 places
``BOOL`` under ``ANY_BIT``, it has boolean semantics and is not treated as a
numeric bit container.

Cross-family widening
~~~~~~~~~~~~~~~~~~~~~

Widening from a bit-string type to an integer type crosses the ``ANY_BIT`` /
``ANY_INT`` boundary and is not part of the IEC 61131-3 standard. IronPLC
supports this as a vendor extension behind the ``--allow-cross-family-widening``
flag (enabled by default in the ``Rusty`` dialect). See
:doc:`/explanation/enabling-dialects-and-features` for how to enable this flag.

When the flag is enabled, a bit-string type can widen to an integer type of
strictly greater bit width:

.. list-table:: Cross-family widening: bit-string to integer (requires ``--allow-cross-family-widening``)
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
   * - **BYTE**
     - No
     - Yes
     - Yes
     - Yes
     - No
     - Yes
     - Yes
     - Yes
   * - **WORD**
     - No
     - No
     - Yes
     - Yes
     - No
     - No
     - Yes
     - Yes
   * - **DWORD**
     - No
     - No
     - No
     - Yes
     - No
     - No
     - No
     - Yes
   * - **LWORD**
     - No
     - No
     - No
     - No
     - No
     - No
     - No
     - No

The reverse direction (integer to bit-string) is never implicit — use explicit
conversion functions such as ``INT_TO_BYTE``.

Bare integer literals (e.g. ``0``) can also be passed to bit-string parameters
when the flag is enabled.

----------------------------
Integer and Real Types
----------------------------

Integer types can implicitly widen to ``REAL`` or ``LREAL`` when the conversion
is lossless — see the integer to real widening matrix above.

Converting from a real type to an integer type is never implicit because it
truncates the fractional part. Use explicit conversion functions such as
``REAL_TO_INT``.

Bare (untyped) integer literals like ``0`` or ``42`` can be passed to ``REAL``
or ``LREAL`` parameters because the literal has no declared type and the
compiler infers the target type.

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
