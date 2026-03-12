===============
Function Blocks
===============

Standard function blocks defined by IEC 61131-3.

Timers
------

.. list-table::
   :header-rows: 1
   :widths: 20 50 30

   * - Function Block
     - Description
     - Status
   * - :doc:`TON <ton>`
     - On-delay timer
     - Supported
   * - :doc:`TOF <tof>`
     - Off-delay timer
     - Supported
   * - :doc:`TP <tp>`
     - Pulse timer
     - Supported

Counters
--------

.. list-table::
   :header-rows: 1
   :widths: 20 50 30

   * - Function Block
     - Description
     - Status
   * - :doc:`CTU <ctu>`
     - Count up
     - Supported
   * - :doc:`CTD <ctd>`
     - Count down
     - Supported
   * - :doc:`CTUD <ctud>`
     - Count up/down
     - Supported

Edge Detection
--------------

.. list-table::
   :header-rows: 1
   :widths: 20 50 30

   * - Function Block
     - Description
     - Status
   * - :doc:`R_TRIG <r-trig>`
     - Rising edge detection
     - Not yet supported
   * - :doc:`F_TRIG <f-trig>`
     - Falling edge detection
     - Not yet supported

Bistable
--------

.. list-table::
   :header-rows: 1
   :widths: 20 50 30

   * - Function Block
     - Description
     - Status
   * - :doc:`SR <sr>`
     - Set-dominant flip-flop
     - Supported
   * - :doc:`RS <rs>`
     - Reset-dominant flip-flop
     - Supported

.. toctree::
   :maxdepth: 1
   :hidden:

   ton
   tof
   tp
   ctu
   ctd
   ctud
   r-trig
   f-trig
   sr
   rs
