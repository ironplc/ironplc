=====
DWORD
=====

Bit string of 32 bits.

.. list-table::
   :widths: 30 70

   * - **Size**
     - 32 bits
   * - **Range**
     - 16#00000000 to 16#FFFFFFFF
   * - **Default**
     - 16#00000000
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Supported

Literals
--------

.. code-block::

   DWORD#16#DEADBEEF
   DWORD#16#00FF00FF

Example
-------

.. playground-with-program::
   :vars: config : DWORD; flag_bit : DWORD; updated : DWORD;

   config := DWORD#16#00FF0000;
   flag_bit := DWORD#16#00000001;
   updated := config OR flag_bit;  (* updated = 16#00FF0001 *)

See Also
--------

- :doc:`word` — 16-bit bit string
- :doc:`lword` — 64-bit bit string
- :doc:`udint` — 32-bit unsigned integer
