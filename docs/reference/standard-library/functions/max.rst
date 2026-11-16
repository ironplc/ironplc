===
MAX
===

Returns the largest of two or more inputs.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
            │   MAX   ├─ OUT
       IN2 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION MAX : ANY_ELEMENTARY
     VAR_INPUT
       IN1 : ANY_ELEMENTARY;
       IN2 : ANY_ELEMENTARY;
       (* ... INn ... *)
     END_VAR
   END_FUNCTION

.. include:: ../../../includes/ordered-selection-input-types.rst

.. include:: ../../../includes/widest-input-return-type.rst

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``IN1``
     - ``ANY_ELEMENTARY``
     - The first value to compare.
   * - ``IN2``
     - ``ANY_ELEMENTARY``
     - The second value to compare.
   * - ``IN3``, ..., ``INn``
     - ``ANY_ELEMENTARY``
     - Further values to compare; the function is extensible, so any number
       of inputs may follow ``IN2``.

.. rubric:: Outputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - Return value
     - ``ANY_ELEMENTARY``
     - The largest of the inputs. If inputs are equal, returns that value.

Description
-----------

Returns the larger of *IN1* and *IN2*. If both inputs are equal,
the function returns that value.

Example
-------

.. playground-with-program::
   :vars: result : DINT;

   result := MAX(10, 20);    (* result = 20 *)
   result := MAX(-5, 3);     (* result = 3 *)

A duration that is never negative:

.. playground-with-program::
   :vars: elapsed : TIME;

   elapsed := MAX(T#-1s, T#0s);    (* elapsed = T#0s *)

See Also
--------

* :doc:`min` — minimum of two values
* :doc:`limit` — clamp to range
* :doc:`sel` — binary selection

References
----------

* IEC 61131-3 §2.5.1.5.5
* `CODESYS: MAX <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_max.html>`_
* `Beckhoff TwinCAT 3: MAX <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528961419.html>`_
* `Fernhill SCADA: MAX <https://www.fernhillsoftware.com/help/iec-61131/common-elements/selection-functions/maximum.html>`_
