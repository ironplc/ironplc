===
ADD
===

Returns the sum of two or more inputs.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
       IN2 ─┤   ADD   ├─ OUT
       IN3 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION ADD : ANY_NUM
     VAR_INPUT
       IN1 : ANY_NUM;
       IN2 : ANY_NUM;
       (* ... additional inputs ... *)
     END_VAR
   END_FUNCTION

The return type matches the input type. ``ADD`` accepts ``SINT``,
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
     - The first addend.
   * - ``IN2``
     - ``ANY_NUM``
     - The second addend.

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
     - The sum of all inputs. Same type as the inputs.

Description
-----------

Returns the sum of all inputs. ``ADD(a, b)`` is the functional form of
the ``+`` operator: ``a + b``. Both forms are equivalent, and
``ADD(a, b, c)`` is ``a + b + c``.

For integer types, overflow behavior wraps around (modular arithmetic).

Time and date overloads
-----------------------

``ADD`` and the ``+`` operator are also defined on the following time
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
     - :doc:`ADD_TIME <add_time>`
   * - ``TIME_OF_DAY``
     - ``TIME``
     - ``TIME_OF_DAY``
     - :doc:`ADD_TOD_TIME <add_tod_time>`
   * - ``DATE_AND_TIME``
     - ``TIME``
     - ``DATE_AND_TIME``
     - :doc:`ADD_DT_TIME <add_dt_time>`

Folding applies here too: ``ADD(t1, t2, t3)`` is ``t1 + t2 + t3``, two ``ADD_TIME`` steps.

Each typed function has a long form over ``LTIME``, ``LDATE``,
``LTIME_OF_DAY`` and ``LDATE_AND_TIME`` (:doc:`ADD_LTIME <add_ltime>`,
:doc:`ADD_LTOD_LTIME <add_ltod_ltime>`, :doc:`ADD_LDT_LTIME
<add_ldt_ltime>`), which applies when either operand is of a long type.

Any other combination of types is an error
(:doc:`P4049 </reference/compiler/problems/P4049>`).

Example
-------

.. playground-with-program::
   :vars: result : DINT;

   result := ADD(10, 20);      (* result = 30 *)
   result := 10 + 20;          (* result = 30, operator form *)
   result := ADD(10, 20, 30);  (* result = 60 *)

See Also
--------

* :doc:`sub` — subtraction
* :doc:`mul` — multiplication
* :doc:`div` — division

References
----------

* IEC 61131-3 §2.5.1.5.3
* `CODESYS: ADD <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_add.html>`_
* `Beckhoff TwinCAT 3: ADD <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/18014401038341259.html>`_
* `Fernhill SCADA: Arithmetic Functions <https://www.fernhillsoftware.com/help/iec-61131/common-elements/functions-arithmetic.html>`_
