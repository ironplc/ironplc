===============
Subrange Types
===============

A subrange type restricts an integer type to a specified range of values.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.3.3.1
   * - **Support**
     - Partial

Syntax
------

.. code-block:: bnf

   TYPE
       type_name : base_type ( lower_bound .. upper_bound ) ;
   END_TYPE

The base type must be an integer type (``SINT``, ``INT``, ``DINT``,
``LINT``, ``USINT``, ``UINT``, ``UDINT``, or ``ULINT``).

Example
-------

.. code-block::

   TYPE
       Percent : INT (0 .. 100);
       Byte_Range : USINT (0 .. 255);
   END_TYPE

   PROGRAM main
       VAR
           level : Percent := 50;
       END_VAR

       level := level + 10;
   END_PROGRAM

Related Problem Codes
---------------------

- :doc:`/reference/compiler/problems/P2024` — Subrange initial value out of bounds

See Also
--------

- :doc:`enumerated-types` — named set of values
- :doc:`int` — base integer type
