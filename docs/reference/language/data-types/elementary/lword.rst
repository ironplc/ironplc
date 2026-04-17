=====
LWORD
=====

Bit string of 64 bits.

.. list-table::
   :widths: 30 70

   * - **Size**
     - 64 bits
   * - **Range**
     - 16#0000000000000000 to 16#FFFFFFFFFFFFFFFF
   * - **Default**
     - 16#0000000000000000
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Supported

Literals
--------

.. code-block::

   LWORD#16#FFFFFFFFFFFFFFFF
   LWORD#16#0000000000000001

Example
-------

.. playground-with-program::
   :vars: data : LWORD; mask : LWORD; filtered : LWORD;

   data := LWORD#16#DEADBEEF12345678;
   mask := LWORD#16#00000000FFFFFFFF;
   filtered := data AND mask;  (* filtered = 16#12345678 *)

Bit Access
----------

Individual bits can be read and written using ``.n`` or ``.%Xn`` syntax
(for example, ``data.40`` or ``data.%X40``). Valid indices are ``0..63``.
See :doc:`/reference/language/structured-text/bit-access`.

See Also
--------

- :doc:`dword` — 32-bit bit string
- :doc:`ulint` — 64-bit unsigned integer
- :doc:`/reference/language/structured-text/bit-access` — selecting an
  individual bit of a bit-string value
