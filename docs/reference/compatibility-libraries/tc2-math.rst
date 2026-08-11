========
Tc2_Math
========

Compatibility library for Beckhoff's TwinCAT 3 ``Tc2_Math`` PLC library:
``LREAL`` math functions. Activated by a project reference or
``--library Tc2_Math``.

Functions
---------

.. list-table::
   :header-rows: 1
   :widths: 20 25 55

   * - Name
     - Signature
     - Description
   * - ``LTRUNC``
     - ``LTRUNC(IN : LREAL) : LREAL``
     - Integer part of *IN*, rounding toward zero. Unlike the IEC
       :doc:`TRUNC </reference/standard-library/functions/trunc>` (which
       returns an integer type), the result stays ``LREAL``, so values
       beyond any integer type's range truncate exactly without clamping.
   * - ``LMOD``
     - ``LMOD(IN1 : LREAL, IN2 : LREAL) : LREAL``
     - Floating modulo returning the signed remainder (IEEE-754
       ``fmod``): the result has the sign of *IN1* and a magnitude less
       than that of *IN2*.
   * - ``MODABS``
     - ``MODABS(IN : LREAL, IM : LREAL) : LREAL``
     - Modulo returning the unsigned representative in ``[0.0, |IM|)`` —
       the wrap-around commonly used for positioning.
   * - ``FRAC``
     - ``FRAC(IN : LREAL) : LREAL``
     - Fractional part of *IN*: keeps the sign of the input, with a
       magnitude less than 1.0.

Example
-------

.. code-block::

   angle := MODABS(IN := rawAngle, IM := 360.0);

See Also
--------

* :doc:`index` — how compatibility libraries are activated
* `Beckhoff TwinCAT 3: Tc2_Math functions <https://infosys.beckhoff.com/content/1033/tcplclib_tc2_math/68440331.html>`_
  — the vendor documentation this interface reproduces
