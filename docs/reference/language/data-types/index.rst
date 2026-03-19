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
   :widths: 20 15 65

   * - Type
     - Size
     - Description
   * - :doc:`BOOL <bool>`
     - 1 bit
     - Boolean
   * - :doc:`SINT <sint>`
     - 8 bits
     - Signed short integer
   * - :doc:`INT <int>`
     - 16 bits
     - Signed integer
   * - :doc:`DINT <dint>`
     - 32 bits
     - Signed double integer
   * - :doc:`LINT <lint>`
     - 64 bits
     - Signed long integer
   * - :doc:`USINT <usint>`
     - 8 bits
     - Unsigned short integer
   * - :doc:`UINT <uint>`
     - 16 bits
     - Unsigned integer
   * - :doc:`UDINT <udint>`
     - 32 bits
     - Unsigned double integer
   * - :doc:`ULINT <ulint>`
     - 64 bits
     - Unsigned long integer
   * - :doc:`REAL <real>`
     - 32 bits
     - Single-precision floating point
   * - :doc:`LREAL <lreal>`
     - 64 bits
     - Double-precision floating point
   * - :doc:`BYTE <byte>`
     - 8 bits
     - Bit string of 8 bits
   * - :doc:`WORD <word>`
     - 16 bits
     - Bit string of 16 bits
   * - :doc:`DWORD <dword>`
     - 32 bits
     - Bit string of 32 bits
   * - :doc:`LWORD <lword>`
     - 64 bits
     - Bit string of 64 bits
   * - :doc:`STRING <string>`
     - Variable
     - Single-byte character string
   * - :doc:`WSTRING <wstring>`
     - Variable
     - Double-byte character string
   * - :doc:`TIME <time>`
     - 32 bits
     - Duration
   * - :doc:`LTIME <ltime>`
     - 64 bits
     - Duration (Edition 3)
   * - :doc:`DATE <date>`
     - 32 bits
     - Calendar date
   * - :doc:`LDATE <ldate>`
     - 64 bits
     - Calendar date (Edition 3)
   * - :doc:`TIME_OF_DAY <time-of-day>`
     - 32 bits
     - Time of day
   * - :doc:`LTIME_OF_DAY <ltime-of-day>`
     - 64 bits
     - Time of day (Edition 3)
   * - :doc:`DATE_AND_TIME <date-and-time>`
     - 64 bits
     - Date and time of day
   * - :doc:`LDATE_AND_TIME <ldate-and-time>`
     - 64 bits
     - Date and time of day (Edition 3)

Derived Types
-------------

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Type
     - Description
   * - :doc:`Enumerated types <enumerated-types>`
     - Named set of values
   * - :doc:`Subrange types <subrange-types>`
     - Integer type restricted to a range
   * - :doc:`Array types <array-types>`
     - Fixed-size indexed collection
   * - :doc:`Structure types <structure-types>`
     - Record with named fields

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
