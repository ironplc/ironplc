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
   * - :doc:`BOOL <elementary/bool>`
     - 1 bit
     - Boolean
   * - :doc:`SINT <elementary/sint>`
     - 8 bits
     - Signed short integer
   * - :doc:`INT <elementary/int>`
     - 16 bits
     - Signed integer
   * - :doc:`DINT <elementary/dint>`
     - 32 bits
     - Signed double integer
   * - :doc:`LINT <elementary/lint>`
     - 64 bits
     - Signed long integer
   * - :doc:`USINT <elementary/usint>`
     - 8 bits
     - Unsigned short integer
   * - :doc:`UINT <elementary/uint>`
     - 16 bits
     - Unsigned integer
   * - :doc:`UDINT <elementary/udint>`
     - 32 bits
     - Unsigned double integer
   * - :doc:`ULINT <elementary/ulint>`
     - 64 bits
     - Unsigned long integer
   * - :doc:`REAL <elementary/real>`
     - 32 bits
     - Single-precision floating point
   * - :doc:`LREAL <elementary/lreal>`
     - 64 bits
     - Double-precision floating point
   * - :doc:`BYTE <elementary/byte>`
     - 8 bits
     - Bit string of 8 bits
   * - :doc:`WORD <elementary/word>`
     - 16 bits
     - Bit string of 16 bits
   * - :doc:`DWORD <elementary/dword>`
     - 32 bits
     - Bit string of 32 bits
   * - :doc:`LWORD <elementary/lword>`
     - 64 bits
     - Bit string of 64 bits
   * - :doc:`STRING <elementary/string>`
     - Variable
     - Single-byte character string
   * - :doc:`WSTRING <elementary/wstring>`
     - Variable
     - Double-byte character string
   * - :doc:`TIME <elementary/time>`
     - 32 bits
     - Duration
   * - :doc:`LTIME <elementary/ltime>`
     - 64 bits
     - Duration (:doc:`Edition 3 </reference/language/edition-support>`)
   * - :doc:`DATE <elementary/date>`
     - 32 bits
     - Calendar date
   * - :doc:`LDATE <elementary/ldate>`
     - 64 bits
     - Calendar date (:doc:`Edition 3 </reference/language/edition-support>`)
   * - :doc:`TIME_OF_DAY <elementary/time-of-day>`
     - 32 bits
     - Time of day
   * - :doc:`LTIME_OF_DAY <elementary/ltime-of-day>`
     - 64 bits
     - Time of day (:doc:`Edition 3 </reference/language/edition-support>`)
   * - :doc:`DATE_AND_TIME <elementary/date-and-time>`
     - 64 bits
     - Date and time of day
   * - :doc:`LDATE_AND_TIME <elementary/ldate-and-time>`
     - 64 bits
     - Date and time of day (:doc:`Edition 3 </reference/language/edition-support>`)

Derived Types
-------------

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Type
     - Description
   * - :doc:`Enumerated types <derived/enumerated-types>`
     - Named set of values
   * - :doc:`Subrange types <derived/subrange-types>`
     - Integer type restricted to a range
   * - :doc:`Array types <derived/array-types>`
     - Fixed-size indexed collection
   * - :doc:`Structure types <derived/structure-types>`
     - Record with named fields
   * - :doc:`Reference types <derived/reference-types>`
     - Pointer to a variable (:doc:`Edition 3 </reference/language/edition-support>`)

.. toctree::
   :maxdepth: 1
   :hidden:
   :caption: Elementary Types

   elementary/bool
   elementary/sint
   elementary/int
   elementary/dint
   elementary/lint
   elementary/usint
   elementary/uint
   elementary/udint
   elementary/ulint
   elementary/real
   elementary/lreal
   elementary/byte
   elementary/word
   elementary/dword
   elementary/lword
   elementary/string
   elementary/wstring
   elementary/time
   elementary/ltime
   elementary/date
   elementary/ldate
   elementary/time-of-day
   elementary/ltime-of-day
   elementary/date-and-time
   elementary/ldate-and-time

.. toctree::
   :maxdepth: 1
   :hidden:
   :caption: Derived Types

   derived/enumerated-types
   derived/subrange-types
   derived/array-types
   derived/structure-types
   derived/reference-types
