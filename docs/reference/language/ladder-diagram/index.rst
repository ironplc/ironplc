===============
Ladder Diagram
===============

Ladder Diagram (LD) is a graphical programming language based on relay
logic diagrams. It represents logic as a series of rungs connecting a
left power rail to a right power rail, with contacts and coils that
implement boolean logic.

.. list-table::
   :header-rows: 1
   :widths: 25 45 30

   * - Element
     - Description
     - Status
   * - :doc:`rungs`
     - Horizontal logic lines
     - Not yet supported
   * - :doc:`contacts`
     - Normally open and normally closed contacts
     - Not yet supported
   * - :doc:`coils`
     - Output, set, and reset coils
     - Not yet supported
   * - :doc:`branches`
     - Parallel and series connections
     - Not yet supported

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 3.2
   * - **Support**
     - Not yet supported

.. toctree::
   :maxdepth: 1
   :hidden:

   rungs
   contacts
   coils
   branches
