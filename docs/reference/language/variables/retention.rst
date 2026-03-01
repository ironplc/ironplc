=========
Retention
=========

Retention qualifiers control whether variables preserve their values
across power cycles and program restarts.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.4.3
   * - **Support**
     - Not yet supported

Qualifiers
----------

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Qualifier
     - Description
   * - ``RETAIN``
     - Value is preserved across power cycles
   * - ``NON_RETAIN``
     - Value is reset to initial value on restart
   * - ``CONSTANT``
     - Value cannot be modified after initialization

Example
-------

.. code-block:: iec61131

   PROGRAM main
       VAR RETAIN
           run_hours : DINT := 0;
       END_VAR
       VAR CONSTANT
           MAX_TEMP : INT := 150;
       END_VAR
       VAR
           current_temp : INT;
       END_VAR

       run_hours := run_hours + 1;
   END_PROGRAM

See Also
--------

- :doc:`declarations` — basic variable syntax
- :doc:`initial-values` — initialization
