========
Branches
========

Branches create parallel and series connections in ladder diagrams,
allowing multiple logic paths within a single rung.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 3.2
   * - **Support**
     - Not yet supported

Description
-----------

**Parallel branches** (OR logic) connect contacts side by side. Power
flows if any branch is TRUE:

::

   |    input_a                         |
   |---| |---+---( output )---|
   |         |                          |
   |    input_b                         |
   |---| |---+
   |                                    |

This is equivalent to ``output := input_a OR input_b`` in Structured Text.

**Series connections** (AND logic) place contacts end to end. Power
flows only if all contacts are TRUE:

::

   |    input_a     input_b     output  |
   |---| |-------| |---( )---|
   |                                    |

This is equivalent to ``output := input_a AND input_b`` in Structured Text.

See Also
--------

- :doc:`rungs` — logic lines
- :doc:`contacts` — input elements
- :doc:`coils` — output elements
