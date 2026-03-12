======
R_TRIG
======

Rising edge detector. Output ``Q`` is ``TRUE`` for one scan cycle when the
input ``CLK`` transitions from ``FALSE`` to ``TRUE``.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.2.3.2
   * - **Support**
     - Supported

Inputs
------

.. list-table::
   :header-rows: 1
   :widths: 20 20 60

   * - Name
     - Type
     - Description
   * - ``CLK``
     - ``BOOL``
     - Signal to monitor for rising edge

Outputs
-------

.. list-table::
   :header-rows: 1
   :widths: 20 20 60

   * - Name
     - Type
     - Description
   * - ``Q``
     - ``BOOL``
     - Edge output (TRUE for one scan on rising edge)

Behavior
--------

The function block stores the previous value of ``CLK``. When ``CLK``
transitions from ``FALSE`` to ``TRUE``, the output ``Q`` is set to ``TRUE``
for one scan cycle. On all subsequent scans where ``CLK`` remains ``TRUE``,
``Q`` is ``FALSE``. When ``CLK`` is ``FALSE``, ``Q`` is always ``FALSE``.

Example
-------

This example detects a rising edge. The input starts ``TRUE``, so ``Q`` is
``TRUE`` on the first scan (transition from the default ``FALSE`` to ``TRUE``).

.. playground::

   PROGRAM main
      VAR
         edge1 : R_TRIG;
         detected : BOOL;
      END_VAR

      edge1(CLK := TRUE, Q => detected);
      (* detected is TRUE on the first scan because CLK rose from FALSE to TRUE *)
   END_PROGRAM

See Also
--------

- :doc:`f-trig` — falling edge detector
