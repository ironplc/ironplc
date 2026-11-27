======
CONCAT
======

Concatenates two or more strings.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
            │ CONCAT  ├─ OUT
       IN2 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION CONCAT : ANY_STRING
     VAR_INPUT
       IN1 : ANY_STRING;
       IN2 : ANY_STRING;
       (* ... INn ... *)
     END_VAR
   END_FUNCTION

The return type matches the input type. ``CONCAT`` accepts ``STRING``.
All inputs must share the same type.

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``IN1``
     - ``ANY_STRING``
     - The first string.
   * - ``IN2``
     - ``ANY_STRING``
     - The string to append to IN1.
   * - ``IN3``, ..., ``INn``
     - ``ANY_STRING``
     - Further strings to append, in order; the function is extensible, so
       any number of inputs may follow ``IN2``.

.. rubric:: Outputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - Return value
     - ``ANY_STRING``
     - The inputs joined in order. Same type as the inputs.

Description
-----------

Returns a new string formed by appending *IN2* to the end of *IN1*, then
each further input in order. ``CONCAT(a, b, c)`` is the same string as
``CONCAT(CONCAT(a, b), c)``.

IEC 61131-3 and RuSTy declare ``CONCAT`` extensible. CODESYS and TwinCAT
declare it with exactly two inputs, so under the ``codesys`` and ``twincat``
dialects a third input is
:doc:`P4018 </reference/compiler/problems/P4018>`; nest the calls instead.
The ``--allow-extensible-concat`` flag selects the extensible form.

Example
-------

.. playground-with-program::
   :vars: result : STRING;

   result := CONCAT('Hello', ' World');    (* result = 'Hello World' *)
   result := CONCAT('A', 'B');             (* result = 'AB' *)
   result := CONCAT('A', 'B', 'C', 'D');   (* result = 'ABCD' *)

See Also
--------

* :doc:`insert` — string insertion
* :doc:`len` — string length
* :doc:`left` — left substring

References
----------

* IEC 61131-3 §2.5.1.5.7
* `CODESYS: CONCAT <https://content.helpme-codesys.com/en/libs/Standard/Current/String-Functions/CONCAT.html>`_
* `Beckhoff TwinCAT 3: CONCAT <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_standard/74411019.html>`_
* `Fernhill SCADA: CONCAT <https://www.fernhillsoftware.com/help/iec-61131/common-elements/string-functions/string-concat.html>`_
