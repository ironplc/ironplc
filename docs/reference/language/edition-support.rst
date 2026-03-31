===============
Edition Support
===============

Some IEC 61131-3 features require enabling a specific edition of the standard.
See :doc:`/explanation/enabling-dialects-and-features` for how to enable an edition in the
compiler or VS Code extension.

Edition 3 (2013) Features
=========================

Edition 3 introduced the following features, which require
``--dialect iec61131-3-ed3`` (CLI) or ``ironplc.dialect``: ``iec61131-3-ed3``
(VS Code).

.. list-table::
   :header-rows: 1
   :widths: 25 25 50

   * - Feature
     - Category
     - Description
   * - :doc:`LTIME <data-types/elementary/ltime>`
     - Data type
     - 64-bit duration
   * - :doc:`LDATE <data-types/elementary/ldate>`
     - Data type
     - 64-bit calendar date
   * - :doc:`LTIME_OF_DAY <data-types/elementary/ltime-of-day>`
     - Data type
     - 64-bit time of day
   * - :doc:`LDATE_AND_TIME <data-types/elementary/ldate-and-time>`
     - Data type
     - 64-bit date and time of day
   * - :doc:`REF_TO <data-types/derived/reference-types>`
     - Data type
     - Reference (pointer) to a variable
   * - ``REF()``
     - Operator
     - Create a reference to a variable
   * - ``^``
     - Operator
     - Dereference a reference
   * - ``NULL``
     - Literal
     - Null reference value

Edition 2 (2003) Features
=========================

All other supported features use the default Edition 2 and require no
additional flags. See :doc:`index` for the complete language reference.
