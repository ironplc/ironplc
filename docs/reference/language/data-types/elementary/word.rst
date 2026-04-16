====
WORD
====

Bit string of 16 bits.

.. list-table::
   :widths: 30 70

   * - **Size**
     - 16 bits
   * - **Range**
     - 16#0000 to 16#FFFF
   * - **Default**
     - 16#0000
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Supported

Literals
--------

.. code-block::

   WORD#16#FFFF
   WORD#16#00FF
   WORD#2#1010101010101010

Example
-------

.. playground-with-program::
   :vars: status_reg : WORD; mask : WORD; masked : WORD;

   status_reg := WORD#16#FF03;
   mask := WORD#16#00FF;
   masked := status_reg AND mask;  (* masked = 16#0003 *)

Bit Access
----------

Individual bits can be read and written using ``.n`` or ``.%Xn`` syntax
(for example, ``status_reg.5`` or ``status_reg.%X5``). Valid indices are
``0..15``. See :doc:`/reference/language/structured-text/bit-access`.

See Also
--------

- :doc:`byte` — 8-bit bit string
- :doc:`dword` — 32-bit bit string
- :doc:`uint` — 16-bit unsigned integer
- :doc:`/reference/language/structured-text/bit-access` — selecting an
  individual bit of a bit-string value
