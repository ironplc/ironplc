===
SUB
===

Returns the difference of two inputs.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
            │   SUB   ├─ OUT
       IN2 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION SUB : ANY_NUM
     VAR_INPUT
       IN1 : ANY_NUM;
       IN2 : ANY_NUM;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``SUB`` accepts ``SINT``,
``INT``, ``DINT``, ``LINT``, ``USINT``, ``UINT``, ``UDINT``, ``ULINT``,
``REAL``, ``LREAL``. Inputs of different numeric types widen to the
wider one, which is also the return type; see
:doc:`/explanation/type-conversions`. The time and date types are
covered by the overloads below.

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``IN1``
     - ``ANY_NUM``
     - The minuend.
   * - ``IN2``
     - ``ANY_NUM``
     - The subtrahend.

.. rubric:: Outputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - Return value
     - ``ANY_NUM``
     - IN1 minus IN2. Same type as the inputs.

Description
-----------

Returns *IN1* minus *IN2*. ``SUB(a, b)`` is the functional form of the
``-`` operator: ``a - b``. Both forms are equivalent.

For integer types, underflow behavior wraps around (modular arithmetic).

Time and date overloads
-----------------------

``SUB`` and the ``-`` operator are also defined on the following time
and date operands (IEC 61131-3 Table 30). Each combination is the typed
function in the last column and computes what it computes.

.. list-table::
   :header-rows: 1
   :widths: 25 25 25 25
   :align: left

   * - IN1
     - IN2
     - Return value
     - Same as
   * - ``TIME``
     - ``TIME``
     - ``TIME``
     - :doc:`SUB_TIME <sub_time>`
   * - ``DATE``
     - ``DATE``
     - ``TIME``
     - :doc:`SUB_DATE_DATE <sub_date_date>`
   * - ``TIME_OF_DAY``
     - ``TIME``
     - ``TIME_OF_DAY``
     - :doc:`SUB_TOD_TIME <sub_tod_time>`
   * - ``TIME_OF_DAY``
     - ``TIME_OF_DAY``
     - ``TIME``
     - :doc:`SUB_TOD_TOD <sub_tod_tod>`
   * - ``DATE_AND_TIME``
     - ``TIME``
     - ``DATE_AND_TIME``
     - :doc:`SUB_DT_TIME <sub_dt_time>`
   * - ``DATE_AND_TIME``
     - ``DATE_AND_TIME``
     - ``TIME``
     - :doc:`SUB_DT_DT <sub_dt_dt>`

``d1 - d2`` on two ``DATE`` values is therefore a ``TIME``, not a ``DATE``.

Each typed function has a long form over ``LTIME``, ``LDATE``,
``LTIME_OF_DAY`` and ``LDATE_AND_TIME`` (:doc:`SUB_LTIME <sub_ltime>`,
:doc:`SUB_LDATE_LDATE <sub_ldate_ldate>`, :doc:`SUB_LTOD_LTIME
<sub_ltod_ltime>`, :doc:`SUB_LTOD_LTOD <sub_ltod_ltod>`,
:doc:`SUB_LDT_LTIME <sub_ldt_ltime>`, :doc:`SUB_LDT_LDT <sub_ldt_ldt>`),
which applies when either operand is of a long type.

Any other combination of types is an error
(:doc:`P4049 </reference/compiler/problems/P4049>`).

Example
-------

.. playground-with-program::
   :vars: result : DINT;

   result := SUB(30, 10);   (* result = 20 *)
   result := 30 - 10;       (* result = 20, operator form *)

See Also
--------

* :doc:`add` — addition
* :doc:`mul` — multiplication
* :doc:`div` — division

References
----------

* IEC 61131-3 §2.5.1.5.3
* `CODESYS: SUB <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_sub.html>`_
* `Beckhoff TwinCAT 3: SUB <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528870027.html>`_
* `Fernhill SCADA: Arithmetic Functions <https://www.fernhillsoftware.com/help/iec-61131/common-elements/functions-arithmetic.html>`_
