=========
ADD_LTIME
=========

Returns the sum of two long durations.

.. include:: ../../../includes/requires-edition3.rst

Signature
---------

.. code-block:: text

            ┌───────────┐
       IN1 ─┤           │
            │ ADD_LTIME ├─ OUT
       IN2 ─┤           │
            └───────────┘

.. code-block:: text

   FUNCTION ADD_LTIME : LTIME
     VAR_INPUT
       IN1 : LTIME;
       IN2 : LTIME;
     END_VAR
   END_FUNCTION

The return type is ``LTIME``.

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``IN1``
     - ``LTIME``
     - The first duration.
   * - ``IN2``
     - ``LTIME``
     - The second duration.

.. rubric:: Outputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - Return value
     - ``LTIME``
     - The sum of IN1 and IN2.

Description
-----------

Returns the sum of two ``LTIME`` durations *IN1* and *IN2*, in 64-bit
milliseconds.

The short form is :doc:`ADD_TIME <add_time>`. An operand of the short
type (``TIME`` where ``LTIME`` is expected) is accepted and widened to
64 bits.

Example
-------

.. playground-with-program::
   :dialect: iec61131-3-ed3
   :vars: result : LTIME;

   result := ADD_LTIME(LTIME#30d, LTIME#30d);   (* result = LTIME#60d *)

See Also
--------

* :doc:`add_time` — the short form
* :doc:`sub_ltime` — subtract long durations
* :doc:`mul_ltime` — scale a long duration
* :doc:`div_ltime` — divide a long duration

References
----------

* IEC 61131-3 Edition 3, Table 30
