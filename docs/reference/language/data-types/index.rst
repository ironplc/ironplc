==========
Data Types
==========

IEC 61131-3 defines a set of elementary data types and mechanisms for
creating derived types.

.. tip::

   Examples on supported data type pages are interactive — edit the code
   and see results in your browser. Powered by the
   `IronPLC Playground <https://playground.ironplc.com>`_.

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
     - Supported
   * - :doc:`LREAL <lreal>`
     - 64 bits
     - Double-precision floating point
     - Supported
   * - :doc:`BYTE <byte>`
     - 8 bits
     - Bit string of 8 bits
     - Supported
   * - :doc:`WORD <word>`
     - 16 bits
     - Bit string of 16 bits
     - Supported
   * - :doc:`DWORD <dword>`
     - 32 bits
     - Bit string of 32 bits
     - Supported
   * - :doc:`LWORD <lword>`
     - 64 bits
     - Bit string of 64 bits
     - Supported
   * - :doc:`STRING <string>`
     - Variable
     - Single-byte character string
     - Not yet supported
   * - :doc:`WSTRING <wstring>`
     - Variable
     - Double-byte character string
     - Not yet supported
   * - :doc:`TIME <time>`
     - 32 bits
     - Duration
     - Supported
   * - :doc:`LTIME <ltime>`
     - 64 bits
     - Duration (Edition 3)
     - Supported
   * - :doc:`DATE <date>`
     - 32 bits
     - Calendar date
     - Supported
   * - :doc:`LDATE <ldate>`
     - 64 bits
     - Calendar date (Edition 3)
     - Supported
   * - :doc:`TIME_OF_DAY <time-of-day>`
     - 32 bits
     - Time of day
     - Supported
   * - :doc:`LTIME_OF_DAY <ltime-of-day>`
     - 64 bits
     - Time of day (Edition 3)
     - Supported
   * - :doc:`DATE_AND_TIME <date-and-time>`
     - 64 bits
     - Date and time of day
     - Supported
   * - :doc:`LDATE_AND_TIME <ldate-and-time>`
     - 64 bits
     - Date and time of day (Edition 3)
     - Supported

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
   ltime
   date
   ldate
   time-of-day
   ltime-of-day
   date-and-time
   ldate-and-time
   enumerated-types
   subrange-types
   array-types
   structure-types
