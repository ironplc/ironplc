===============
Edition Support
===============

Some IEC 61131-3 features require enabling a specific edition of the standard.
See :doc:`/explanation/enabling-features` for how to enable an edition in the
compiler or VS Code extension.

Edition 3 (2013) Features
=========================

Edition 3 introduced the following features, which require
``--std-iec-61131-3=2013`` (CLI) or ``ironplc.std61131Version``: ``2013``
(VS Code).

.. list-table::
   :header-rows: 1
   :widths: 25 25 50

   * - Feature
     - Category
     - Description
   * - :doc:`LTIME <data-types/ltime>`
     - Data type
     - 64-bit duration
   * - :doc:`LDATE <data-types/ldate>`
     - Data type
     - 64-bit calendar date
   * - :doc:`LTIME_OF_DAY <data-types/ltime-of-day>`
     - Data type
     - 64-bit time of day
   * - :doc:`LDATE_AND_TIME <data-types/ldate-and-time>`
     - Data type
     - 64-bit date and time of day

Edition 2 (2003) Features
=========================

All other supported features use the default Edition 2 and require no
additional flags. See :doc:`index` for the complete language reference.
