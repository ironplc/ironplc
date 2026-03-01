==========
Data Types
==========

IEC 61131-3 defines a set of elementary data types and mechanisms for
creating derived types.

Elementary Types
----------------

.. list-table::
   :header-rows: 1
   :widths: 15 15 40 30

   * - Type
     - Size
     - Description
     - Status
   * - :doc:`BOOL <bool>`
     - 1 bit
     - Boolean
     - Supported
   * - :doc:`SINT <sint>`
     - 8 bits
     - Signed short integer
     - Supported
   * - :doc:`INT <int>`
     - 16 bits
     - Signed integer
     - Supported
   * - :doc:`DINT <dint>`
     - 32 bits
     - Signed double integer
     - Supported
   * - :doc:`LINT <lint>`
     - 64 bits
     - Signed long integer
     - Supported
   * - :doc:`USINT <usint>`
     - 8 bits
     - Unsigned short integer
     - Supported
   * - :doc:`UINT <uint>`
     - 16 bits
     - Unsigned integer
     - Supported
   * - :doc:`UDINT <udint>`
     - 32 bits
     - Unsigned double integer
     - Supported
   * - :doc:`ULINT <ulint>`
     - 64 bits
     - Unsigned long integer
     - Supported
   * - :doc:`REAL <real>`
     - 32 bits
     - Single-precision floating point
     - Not yet supported
   * - :doc:`LREAL <lreal>`
     - 64 bits
     - Double-precision floating point
     - Not yet supported
   * - :doc:`BYTE <byte>`
     - 8 bits
     - Bit string of 8 bits
     - Not yet supported
   * - :doc:`WORD <word>`
     - 16 bits
     - Bit string of 16 bits
     - Not yet supported
   * - :doc:`DWORD <dword>`
     - 32 bits
     - Bit string of 32 bits
     - Not yet supported
   * - :doc:`LWORD <lword>`
     - 64 bits
     - Bit string of 64 bits
     - Not yet supported
   * - :doc:`STRING <string>`
     - Variable
     - Single-byte character string
     - Not yet supported
   * - :doc:`WSTRING <wstring>`
     - Variable
     - Double-byte character string
     - Not yet supported
   * - :doc:`TIME <time>`
     - 64 bits
     - Duration
     - Not yet supported
   * - :doc:`DATE <date>`
     - —
     - Calendar date
     - Not yet supported
   * - :doc:`TIME_OF_DAY <time-of-day>`
     - —
     - Time of day
     - Not yet supported
   * - :doc:`DATE_AND_TIME <date-and-time>`
     - —
     - Date and time of day
     - Not yet supported

Derived Types
-------------

.. list-table::
   :header-rows: 1
   :widths: 25 45 30

   * - Type
     - Description
     - Status
   * - :doc:`Enumerated types <enumerated-types>`
     - Named set of values
     - Partial
   * - :doc:`Subrange types <subrange-types>`
     - Integer type restricted to a range
     - Partial
   * - :doc:`Array types <array-types>`
     - Fixed-size indexed collection
     - Not yet supported
   * - :doc:`Structure types <structure-types>`
     - Record with named fields
     - Partial

.. toctree::
   :maxdepth: 1
   :hidden:

   bool
   sint
   int
   dint
   lint
   usint
   uint
   udint
   ulint
   real
   lreal
   byte
   word
   dword
   lword
   string
   wstring
   time
   date
   time-of-day
   date-and-time
   enumerated-types
   subrange-types
   array-types
   structure-types
