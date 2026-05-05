=====
TRUNC
=====

Truncates a real (floating-point) value toward zero, removing the
fractional part and returning an integer.

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤  TRUNC  ├─ OUT
           └─────────┘

.. code-block:: text

   FUNCTION TRUNC : ANY_INT
     VAR_INPUT
       IN : ANY_REAL;
     END_VAR
   END_FUNCTION

``TRUNC`` accepts ``REAL`` or ``LREAL`` for *IN*. The return type
is determined by the variable being assigned to: ``SINT``, ``INT``,
``DINT``, or ``LINT``.

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``IN``
     - ``ANY_REAL``
     - The real value to truncate.

.. rubric:: Outputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - Return value
     - ``ANY_INT``
     - IN with its fractional part removed. The integer type is determined by the assignment target.

Description
-----------

``TRUNC`` removes the fractional part of a real number, truncating toward
zero. This means positive values are rounded down and negative values
are rounded up (toward zero).

- ``TRUNC(3.7)`` returns ``3``
- ``TRUNC(-3.7)`` returns ``-3``
- ``TRUNC(0.9)`` returns ``0``

The return type is determined by the variable being assigned to.

Example
-------

.. playground-with-program::
   :vars: result : DINT; neg_result : DINT;

   result := TRUNC(REAL#3.7);       (* result = 3 *)
   neg_result := TRUNC(REAL#-3.7);  (* neg_result = -3 *)

See Also
--------

* :doc:`type-conversions` — explicit type conversion functions
* :doc:`abs` — absolute value

References
----------

* IEC 61131-3 §2.5.1.5.2
* `CODESYS: TRUNC <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_trunc.html>`_
* `Beckhoff TwinCAT 3: TRUNC <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2529079691.html>`_
* `Fernhill SCADA: TRUNC <https://www.fernhillsoftware.com/help/iec-61131/common-elements/conversion-functions/truncation.html>`_
