======
STRING
======

Single-byte character string with a maximum length.

.. list-table::
   :widths: 30 70

   * - **Size**
     - Variable (default max 254 characters)
   * - **Default**
     - ``''`` (empty string)
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Not yet supported

Literals
--------

.. code-block::

   'Hello, world!'
   'It''s escaped'
   STRING#'typed literal'

The maximum length can be specified in the declaration:

.. code-block::

   VAR
       name : STRING[50];   (* max 50 characters *)
       msg  : STRING;       (* default max length *)
   END_VAR

Constant Length (Vendor Extension)
----------------------------------

.. include:: ../../../../includes/requires-vendor-extension.rst

With the ``--allow-constant-type-params`` flag (or ``--allow-all``), you can
use a global constant for the maximum length instead of a literal:

.. code-block::

   VAR_GLOBAL CONSTANT
       MAX_NAME_LEN : INT := 50;
   END_VAR

   FUNCTION_BLOCK fb1
       VAR_EXTERNAL CONSTANT
           MAX_NAME_LEN : INT;
       END_VAR
       VAR
           name : STRING[MAX_NAME_LEN];
       END_VAR
   END_FUNCTION_BLOCK

See Also
--------

- :doc:`wstring` — double-byte character string
- :doc:`/reference/standard-library/functions/len` — string length
- :doc:`/reference/standard-library/functions/concat` — string concatenation
