==========
Bit Access
==========

Bit access selects a single bit of an integer-typed or bit-string-typed
variable. The selected bit reads and writes as a :doc:`BOOL
</reference/language/data-types/elementary/bool>`. The Edition 3
partial-access syntax generalizes this to also select a byte, word, double
word, or long word of a wider value.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.4.1.2 (partial access)
   * - **Support**
     - Supported (``.n`` short form);
       :doc:`Edition 3 </reference/language/edition-support>` (``.%Xn``,
       ``.%Bn``, ``.%Wn``, ``.%Dn``, ``.%Ln``)

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
``--allow-partial-access-syntax``.

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

IEC 61131-3:2013 also defines partial access at wider granularities. Each
form selects slice ``n`` of the base value, counting from the least
significant end, and reads and writes as the bit-string type of that width:

.. list-table::
   :header-rows: 1
   :widths: 15 15 20 50

   * - Form
     - Width
     - Result type
     - Example
   * - ``.%Bn``
     - 8 bits
     - :doc:`BYTE </reference/language/data-types/elementary/byte>`
     - ``my_dword.%B2`` — bits ``16..23`` of a ``DWORD``
   * - ``.%Wn``
     - 16 bits
     - :doc:`WORD </reference/language/data-types/elementary/word>`
     - ``my_lword.%W1`` — bits ``16..31`` of an ``LWORD``
   * - ``.%Dn``
     - 32 bits
     - :doc:`DWORD </reference/language/data-types/elementary/dword>`
     - ``my_lword.%D1`` — bits ``32..63`` of an ``LWORD``
   * - ``.%Ln``
     - 64 bits
     - :doc:`LWORD </reference/language/data-types/elementary/lword>`
     - ``my_lword.%L0`` — all 64 bits of an ``LWORD``

Slice ``n`` covers bits ``n * width`` through ``(n + 1) * width - 1`` of
the base value. Index ``0`` is the least significant slice.

The base may be any integer or bit-string type at least as wide as the
slice. The valid index range depends on both widths:

.. list-table::
   :header-rows: 1
   :widths: 24 19 19 19 19

   * - Base type width
     - ``.%Bn``
     - ``.%Wn``
     - ``.%Dn``
     - ``.%Ln``
   * - 8-bit
     - ``0``
     - —
     - —
     - —
   * - 16-bit
     - ``0..1``
     - ``0``
     - —
     - —
   * - 32-bit
     - ``0..3``
     - ``0..1``
     - ``0``
     - —
   * - 64-bit
     - ``0..7``
     - ``0..3``
     - ``0..1``
     - ``0``

An index outside the valid range, or a slice wider than the base type,
raises :doc:`P4025 </reference/compiler/problems/P4025>`.

Writing to a slice replaces only the bits of that slice; the other bits of
the base value keep their values. Like ``.%Xn``, these forms are gated
behind ``--allow-partial-access-syntax`` and raise
:doc:`P4033 </reference/compiler/problems/P4033>` without it. Unlike
``.%Xn``, they have no Edition 2 short form.

.. code-block::

   PROGRAM main
       VAR
           packet : DWORD := 16#AABBCCDD;
           lo     : BYTE;
           hi     : WORD;
       END_VAR

       lo := packet.%B0;         (* 16#DD — least significant byte *)
       hi := packet.%W1;         (* 16#AABB — upper 16 bits *)
       packet.%B1 := 16#FF;      (* packet becomes 16#AABBFFDD *)
   END_PROGRAM

The plc2plc renderer preserves these forms on output, since they have no
short-form equivalent.

See Also
--------

- :doc:`assignment` — assignment statement
- :doc:`/reference/language/data-types/elementary/byte` — 8-bit bit string
- :doc:`/reference/language/data-types/elementary/word` — 16-bit bit string
- :doc:`/reference/language/data-types/elementary/dword` — 32-bit bit string
- :doc:`/reference/language/data-types/elementary/lword` — 64-bit bit string
- :doc:`/reference/compiler/problems/P4025` — bit index out of range
- :doc:`/reference/compiler/problems/P4033` — partial-access syntax disabled
