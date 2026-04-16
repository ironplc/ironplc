==========
Bit Access
==========

Bit access selects a single bit of an integer-typed or bit-string-typed
variable. The selected bit reads and writes as a :doc:`BOOL
</reference/language/data-types/elementary/bool>`.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.4.1.2 (partial access)
   * - **Support**
     - Supported (short form); bit form of partial-access syntax supported

Syntax
------

IronPLC accepts two equivalent forms for bit access:

.. list-table::
   :header-rows: 1
   :widths: 25 25 50

   * - Form
     - Example
     - Availability
   * - ``variable.n``
     - ``my_byte.3``
     - Always supported (Edition 2 short form)
   * - ``variable.%Xn``
     - ``my_byte.%X3``
     - Edition 3 partial-access syntax; see below

Both forms denote the same bit and produce the same runtime behavior — they
differ only in surface syntax. Bit indices are zero-based, with ``0`` being
the least significant bit.

Bit access composes with other variable references. The bit suffix may
follow any symbolic variable, including array subscripts and structure
field accesses:

.. code-block::

   my_byte.3              (* simple variable *)
   my_array[i].3          (* array element *)
   my_record.field.3      (* structure field *)

Valid Base Types
----------------

Bit access is valid on any integer or bit-string type:

.. list-table::
   :header-rows: 1
   :widths: 40 30 30

   * - Type family
     - Types
     - Valid bit indices
   * - 8-bit
     - ``SINT``, ``USINT``, ``BYTE``
     - ``0..7``
   * - 16-bit
     - ``INT``, ``UINT``, ``WORD``
     - ``0..15``
   * - 32-bit
     - ``DINT``, ``UDINT``, ``DWORD``
     - ``0..31``
   * - 64-bit
     - ``LINT``, ``ULINT``, ``LWORD``
     - ``0..63``

Accessing a bit outside the valid range raises
:doc:`P4025 </reference/compiler/problems/P4025>`.

Example
-------

.. playground::

   PROGRAM main
       VAR
           flags : BYTE := 2#00000101;
           bit0  : BOOL;
           bit2  : BOOL;
       END_VAR

       bit0 := flags.0;        (* TRUE  — least significant bit *)
       bit2 := flags.2;        (* TRUE  *)
       flags.1 := TRUE;        (* set bit 1; flags becomes 2#00000111 *)
   END_PROGRAM

Edition 3 Partial-Access Syntax
-------------------------------

.. include:: ../../../includes/requires-edition3.rst

IEC 61131-3:2013 adds the explicit form ``variable.%Xn`` for bit access.
Semantically it is identical to the ``.n`` short form — IronPLC lowers both
to the same representation. The Edition 3 form is gated behind
``--allow-partial-access-syntax`` and is enabled by default under
``--dialect=iec61131-3-ed3`` and ``--dialect=rusty``.

Using ``.%Xn`` without the flag raises
:doc:`P4033 </reference/compiler/problems/P4033>`.

.. code-block::

   PROGRAM main
       VAR
           my_byte_array : ARRAY[0..1] OF BYTE := [2#00000101, 2#00000000];
           r             : BOOL;
       END_VAR

       r := my_byte_array[0].%X0;     (* TRUE *)
       my_byte_array[0].%X1 := TRUE;  (* write bit 1 *)
   END_PROGRAM

The plc2plc renderer normalizes both surface forms to ``.n`` on output; the
chosen bit index is preserved.

Byte / Word / Dword / Lword Partial Access
------------------------------------------

IEC 61131-3:2013 also defines partial access at wider granularities:

.. list-table::
   :header-rows: 1
   :widths: 20 35 45

   * - Form
     - Selects
     - Status
   * - ``.%Bn``
     - Byte ``n`` of a wider value
     - Not yet supported
   * - ``.%Wn``
     - Word ``n`` of a wider value
     - Not yet supported
   * - ``.%Dn``
     - Double word ``n`` of a wider value
     - Not yet supported
   * - ``.%Ln``
     - Long word ``n`` of a wider value
     - Not yet supported

These forms return a non-``BOOL`` view of the underlying value and require
additional codegen work. Until they land, a source that uses them raises
:doc:`P4033 </reference/compiler/problems/P4033>` when the flag is off and
``P0003`` (``Unmatched character sequence``) when the flag is on.

See Also
--------

- :doc:`assignment` — assignment statement
- :doc:`/reference/language/data-types/elementary/byte` — 8-bit bit string
- :doc:`/reference/language/data-types/elementary/word` — 16-bit bit string
- :doc:`/reference/language/data-types/elementary/dword` — 32-bit bit string
- :doc:`/reference/language/data-types/elementary/lword` — 64-bit bit string
- :doc:`/reference/compiler/problems/P4025` — bit index out of range
- :doc:`/reference/compiler/problems/P4033` — partial-access syntax disabled
