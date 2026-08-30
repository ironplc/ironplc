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
     - Supported
   * - **Encoding**
     - UTF-16LE, Basic Multilingual Plane only

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

Encoding
--------

``WSTRING`` stores UTF-16LE code units. Only the Basic Multilingual Plane is
supported: a code point above U+FFFF has no representation, so surrogate pairs
are not formed. Lengths — the declared maximum, and the value ``LEN`` returns —
count code units, not bytes and not user-perceived characters.

The maximum declared length is 65,535 code units.

``STRING`` and ``WSTRING`` cannot be mixed. Assigning one to the other is a
compile error (:doc:`/reference/compiler/problems/P4034`); the runtime also
traps a mixed operation that reaches it.

See Also
--------

- :doc:`string` — single-byte character string
