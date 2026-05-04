=========
DT_TO_TOD
=========

Extracts the time-of-day portion from a date-and-time value.

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤DT_TO_TOD├─ OUT
           └─────────┘

.. code-block:: text

   FUNCTION DT_TO_TOD : TIME_OF_DAY
     VAR_INPUT
       IN : DATE_AND_TIME;
     END_VAR
   END_FUNCTION

The return type is ``TIME_OF_DAY``. *IN* is ``DATE_AND_TIME``.

Description
-----------

Extracts the time-of-day portion from *IN*, returning a ``TIME_OF_DAY``
value in milliseconds since midnight.

The long-form alias ``DATE_AND_TIME_TO_TIME_OF_DAY`` is also supported.

Example
-------

.. playground-with-program::
   :vars: result : TIME_OF_DAY;

   result := DT_TO_TOD(DT#2000-01-01-12:00:00);

See Also
--------

* :doc:`dt_to_date` — extract date from datetime
* :doc:`concat_date_tod` — combine date and time-of-day

References
----------

* IEC 61131-3 §2.5.1.5
* `CODESYS: Operators (overview) <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_struct_reference_operators.html>`_
* `Beckhoff TwinCAT 3: Type conversion (overview) <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/63050398781277579.html>`_
* `Fernhill SCADA: Type Casts <https://www.fernhillsoftware.com/help/iec-61131/common-elements/conversion-functions/type-casts.html>`_
