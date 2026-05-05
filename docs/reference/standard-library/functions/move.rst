====
MOVE
====

Copies the input value to the output (assignment).

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤  MOVE   ├─ OUT
           └─────────┘

.. code-block:: text

   FUNCTION MOVE : ANY
     VAR_INPUT
       IN : ANY;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``MOVE`` accepts any type
(``ANY``); IronPLC supports ``SINT``, ``INT``, ``DINT``, ``LINT``,
``USINT``, ``UINT``, ``UDINT``, ``ULINT``, ``REAL``, ``LREAL``.

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``IN``
     - ``ANY``
     - The value to copy.

.. rubric:: Outputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - Return value
     - ``ANY``
     - A copy of IN. Same type as IN.

Description
-----------

Copies the value of *IN* to the output. ``MOVE`` is the functional form
of the ``:=`` assignment operator. It is useful when an explicit function
call is preferred over the assignment syntax, for example as an argument
to other functions.

Example
-------

.. playground-with-program::
   :vars: result : DINT;

   result := MOVE(42);       (* result = 42 *)

See Also
--------

* :doc:`sel` — binary selection
* :doc:`limit` — clamp to range

References
----------

* IEC 61131-3 §2.5.1.5.4
* `CODESYS: MOVE <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_move.html>`_
* `Beckhoff TwinCAT 3: MOVE <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528886155.html>`_

.. Fernhill SCADA does not have a dedicated MOVE page (assignment is
   covered by the language reference rather than as a function).
