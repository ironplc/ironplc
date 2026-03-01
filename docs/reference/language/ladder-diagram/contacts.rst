========
Contacts
========

Contacts are input elements in ladder diagrams that read the state of a
boolean variable. Power flows through a contact when its condition is
met.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 3.2
   * - **Support**
     - Not yet supported

Contact Types
-------------

.. list-table::
   :header-rows: 1
   :widths: 25 25 50

   * - Type
     - Symbol
     - Description
   * - Normally open
     - ``--| |--``
     - Passes power when the associated variable is TRUE
   * - Normally closed
     - ``--|/|--``
     - Passes power when the associated variable is FALSE
   * - Positive transition
     - ``--|P|--``
     - Passes power on rising edge (FALSE to TRUE)
   * - Negative transition
     - ``--|N|--``
     - Passes power on falling edge (TRUE to FALSE)

Description
-----------

A normally open contact is equivalent to reading a boolean variable
directly. A normally closed contact is equivalent to applying ``NOT``
to the variable.

See Also
--------

- :doc:`coils` — output elements
- :doc:`rungs` — logic lines
- :doc:`/reference/standard-library/function-blocks/r-trig` — rising edge in ST
- :doc:`/reference/standard-library/function-blocks/f-trig` — falling edge in ST
