====
BYTE
====

Bit string of 8 bits.

.. list-table::
   :widths: 30 70

   * - **Size**
     - 8 bits
   * - **Range**
     - 16#00 to 16#FF
   * - **Default**
     - 16#00
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Supported

Literals
--------

.. code-block::

   BYTE#16#FF
   BYTE#2#11001010
   BYTE#8#377

Example
-------

.. playground-with-program::
   :vars: flags : BYTE; mask : BYTE; result : BYTE;

   flags := BYTE#16#A5;
   mask := BYTE#16#0F;
   result := flags AND mask;  (* result = 16#05 *)

Bit Access
----------

Individual bits can be read and written using ``.n`` or ``.%Xn`` syntax
(for example, ``flags.3`` or ``flags.%X3``). Valid indices are ``0..7``.
See :doc:`/reference/language/structured-text/bit-access`.

See Also
--------

- :doc:`word` — 16-bit bit string
- :doc:`usint` — 8-bit unsigned integer
- :doc:`/reference/language/structured-text/bit-access` — selecting an
  individual bit of a bit-string value
