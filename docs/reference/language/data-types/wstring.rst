=======
WSTRING
=======

Double-byte (wide) character string with a maximum length.

.. list-table::
   :widths: 30 70

   * - **Size**
     - Variable (default max 254 characters)
   * - **Default**
     - ``""`` (empty string)
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Not yet supported

Literals
--------

.. code-block::

   "Hello, world!"
   "Double-byte string"
   WSTRING#"typed literal"

The maximum length can be specified in the declaration:

.. code-block::

   VAR
       name : WSTRING[50];   (* max 50 characters *)
       msg  : WSTRING;       (* default max length *)
   END_VAR

See Also
--------

- :doc:`string` — single-byte character string
