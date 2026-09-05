=========
Functions
=========

Extension functions provided by IronPLC. These are not part of the
IEC 61131-3 standard but are widely supported across PLC environments.

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Function
     - Description
   * - :doc:`ADR <adr>`
     - Address of a variable as a typed pointer (requires ``--allow-adr``)
   * - :doc:`SIZEOF <sizeof>`
     - Size in bytes of a variable or type (requires ``--allow-sizeof``)
   * - :doc:`__ISVALIDREF <isvalidref>`
     - Whether a ``REFERENCE TO`` variable is bound (requires ``--allow-reference-to``)

.. toctree::
   :maxdepth: 1
   :hidden:

   adr
   sizeof
   isvalidref
