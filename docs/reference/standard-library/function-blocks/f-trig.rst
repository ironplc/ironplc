======
F_TRIG
======

Falling edge detector. Output ``Q`` is ``TRUE`` for one scan cycle when the
input ``CLK`` transitions from ``TRUE`` to ``FALSE``.

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
     - Signal to monitor for falling edge

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
     - Edge output (TRUE for one scan on falling edge)

Behavior
--------

The function block stores the previous value of ``CLK``. When ``CLK``
transitions from ``TRUE`` to ``FALSE``, the output ``Q`` is set to ``TRUE``
for one scan cycle. On all subsequent scans where ``CLK`` remains ``FALSE``,
``Q`` is ``FALSE``. When ``CLK`` is ``TRUE``, ``Q`` is always ``FALSE``.

Example
-------

This example shows that no falling edge is detected when ``CLK`` starts
``FALSE`` (no transition occurred — the default was already ``FALSE``).

.. playground::

   PROGRAM main
      VAR
         edge1 : F_TRIG;
         detected : BOOL;
      END_VAR

      edge1(CLK := FALSE, Q => detected);
      (* detected is FALSE because CLK did not transition from TRUE to FALSE *)
   END_PROGRAM

See Also
--------

- :doc:`r-trig` — rising edge detector
