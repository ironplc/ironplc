====
BOOL
====

Boolean value: ``TRUE`` or ``FALSE``.

.. list-table::
   :widths: 30 70

   * - **Size**
     - 1 bit
   * - **Values**
     - ``TRUE``, ``FALSE``
   * - **Default**
     - ``FALSE``
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Supported

Literals
--------

.. code-block::

   TRUE
   FALSE
   BOOL#1
   BOOL#0

Example
-------

.. playground-with-program::
   :vars: sensor : BOOL; enabled : BOOL; run : BOOL;

   sensor := TRUE;
   enabled := TRUE;
   run := sensor AND enabled;  (* run = TRUE *)

See Also
--------

- :doc:`/reference/language/structured-text/logical-operators` — AND, OR, XOR, NOT
