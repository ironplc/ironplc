==========
DT_TO_DATE
==========

Extracts the date portion from a date-and-time value.

Signature
---------

.. code-block:: text

           ┌──────────┐
       IN ─┤DT_TO_DATE├─ OUT
           └──────────┘

.. code-block:: text

   FUNCTION DT_TO_DATE : DATE
     VAR_INPUT
       IN : DATE_AND_TIME;
     END_VAR
   END_FUNCTION

The return type is ``DATE``. *IN* is ``DATE_AND_TIME``.

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``IN``
     - ``DATE_AND_TIME``
     - The date-and-time to extract the date from.

.. rubric:: Outputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - Return value
     - ``DATE``
     - The date portion of IN, with the time-of-day stripped.

Description
-----------

Extracts the date portion from *IN* by stripping the time-of-day component,
returning a ``DATE`` value representing midnight of the same day.

The long-form alias ``DATE_AND_TIME_TO_DATE`` is also supported.

Example
-------

.. playground-with-program::
   :vars: result : DATE;

   result := DT_TO_DATE(DT#2000-01-01-12:00:00);

See Also
--------

* :doc:`dt_to_tod` — extract time-of-day from datetime
* :doc:`concat_date_tod` — combine date and time-of-day

References
----------

* IEC 61131-3 §2.5.1.5
* `CODESYS: Operators (overview) <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_struct_reference_operators.html>`_
* `Beckhoff TwinCAT 3: Type conversion (overview) <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/63050398781277579.html>`_
* `Fernhill SCADA: Type Casts <https://www.fernhillsoftware.com/help/iec-61131/common-elements/conversion-functions/type-casts.html>`_
