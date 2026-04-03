================
Enumerated Types
================

An enumerated type defines a named set of values.

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
       type_name : ( value1, value2, ... ) ;
   END_TYPE

Example
-------

.. code-block::

   TYPE
       TrafficLight : (Red, Yellow, Green);
   END_TYPE

   PROGRAM main
       VAR
           state : TrafficLight := Red;
       END_VAR

       IF state = Green THEN
           state := Yellow;
       END_IF;
   END_PROGRAM

Enumerated values must be unique within the type. Values can optionally
include a type qualifier:

.. code-block::

   TYPE
       Color : (Red, Green, Blue) INT;
   END_TYPE

Related Problem Codes
---------------------

- :doc:`/reference/compiler/problems/P2003` — Duplicate enumeration value

See Also
--------

- :doc:`subrange-types` — restrict an integer to a range
