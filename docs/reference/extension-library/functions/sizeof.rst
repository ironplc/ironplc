======
SIZEOF
======

Returns the size in bytes of a variable or type.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Not part of the standard (vendor extension)
   * - **Support**
     - Supported (requires ``--allow-sizeof`` or ``--dialect rusty``)

Signatures
----------

.. list-table::
   :header-rows: 1
   :widths: 10 30 30 30

   * - #
     - Input (IN)
     - Return Type
     - Support
   * - 1
     - ``ANY``
     - ``ANY_INT``
     - Supported

Description
-----------

``SIZEOF`` returns the size in bytes of the argument's type as a
compile-time constant. It is not part of the IEC 61131-3 standard but is
a widely supported vendor extension available in CODESYS, TwinCAT/Beckhoff,
and RuSTy. It is commonly used in buffer management functions that work with
``REF_TO`` pointers, such as those in the OSCAT library.

For elementary types, ``SIZEOF`` returns the number of bytes the type
occupies:

- ``BOOL`` → 1
- ``INT`` → 2
- ``DINT`` / ``DWORD`` / ``REAL`` → 4
- ``LINT`` / ``LREAL`` → 8

For arrays, ``SIZEOF`` returns the total number of bytes occupied by all
elements (element count × element size).

Enabling
--------

``SIZEOF`` is a vendor extension and must be explicitly enabled:

.. code-block:: shell

   ironplcc check --allow-sizeof main.st

Or use the RuSTy dialect which enables all vendor extensions:

.. code-block:: shell

   ironplcc check --dialect rusty main.st

See :doc:`/explanation/enabling-dialects-and-features` for more information
about dialects and feature flags.

Example
-------

.. playground-with-program::
   :vars: x : INT; arr : ARRAY[1..10] OF INT;

   s := SIZEOF(x);     (* s = 2 *)
   s := SIZEOF(arr);   (* s = 20 *)

See Also
--------

- :doc:`/explanation/enabling-dialects-and-features` — enabling vendor extensions
