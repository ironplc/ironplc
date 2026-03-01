=====
Rungs
=====

A rung is a horizontal logic line in a ladder diagram, connecting a left
power rail to a right power rail. Each rung represents a single logic
path containing contacts, coils, and function block invocations.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 3.2
   * - **Support**
     - Not yet supported

Description
-----------

Power flows from the left rail through a series of contacts. If all
contacts in the path evaluate to TRUE, power reaches the right rail
and activates the coil.

::

   |    start_button     motor_output    |
   |---| |---+---( )---|
   |             |
   |    hold     |
   |---| |---+
   |                                    |

Each rung is evaluated once per scan cycle.

See Also
--------

- :doc:`contacts` — input elements
- :doc:`coils` — output elements
- :doc:`branches` — parallel connections
