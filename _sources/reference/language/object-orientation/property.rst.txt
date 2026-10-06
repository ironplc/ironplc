========
PROPERTY
========

``PROPERTY`` declares a named value on a function block type that is read
and written like a variable, but runs code when it is: a ``GET`` accessor
computes the value on a read, and a ``SET`` accessor handles a write. A
property can have either accessor or both, so it can be read-only,
write-only, or read-write. A property declaration is terminated by
``END_PROPERTY``.

.. |keyword| replace:: ``PROPERTY``
.. |flag| replace:: ``--allow-fb-inheritance``
.. include:: /includes/oop-keyword-flag.rst

.. note::

   ``END_PROPERTY``, ``END_GET`` and ``END_SET`` are gated by the same flag.
   ``GET`` and ``SET`` are keywords only where an accessor starts, directly
   inside a property. Everywhere else they stay ordinary identifiers, even
   with the flag enabled, so a variable or an input named ``SET`` keeps
   working.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Object-oriented extension, as used by CODESYS and TwinCAT
   * - **Support**
     - Parsed and analyzed: a function block that declares properties
       compiles, and the accessor bodies are checked. Reading or writing a
       property is not supported yet (see :ref:`property-limitations`).
       Enable with ``--allow-fb-inheritance``; see
       :doc:`/explanation/enabling-dialects-and-features`.

Syntax
------

A property is declared inside the function block it belongs to, between the
function block's body and ``END_FUNCTION_BLOCK``, in any order with its
:doc:`methods <method>`:

.. code-block:: bnf

   PROPERTY property_name : type
       [GET
           variable_declarations
           statement_list
       END_GET]
       [SET
           variable_declarations
           statement_list
       END_SET]
   END_PROPERTY

Inside ``GET``, the property name is the result: assign it, as a
:doc:`method <method>` with a return type assigns its own name. Inside
``SET``, the property name holds the value being written. Each accessor can
declare its own local variables with ``VAR``.

In a TwinCAT ``.TcPOU`` file, a property is a ``<Property>`` element with
``<Get>`` and ``<Set>`` children. IronPLC reads both forms.

Example
-------

.. code-block::

   FUNCTION_BLOCK FB_Motor
       VAR
           _speed : REAL;
       END_VAR

       PROPERTY Speed : REAL
           GET
               Speed := _speed;
           END_GET
           SET
               _speed := Speed;
           END_SET
       END_PROPERTY
   END_FUNCTION_BLOCK

.. _property-limitations:

Current limitations
-------------------

A property can be declared, but not used yet. Reading it
(``x := motor.Speed``) or writing it (``motor.Speed := 2.0``), from outside
the function block or by its bare name inside it, reports
:doc:`P9999 </reference/compiler/problems/P9999>`. Until that is supported,
call a :doc:`method <method>` or access a field of the function block
instead.

Access modifiers such as ``PROPERTY PUBLIC`` are not parsed yet, and neither
are properties declared in an :doc:`interface`.

See Also
--------

- :doc:`method`: declare an operation on a function block type
- :doc:`interface`: declare a set of method signatures
- :doc:`/explanation/object-orientation`: inheritance, interfaces, and
  abstract types explained
- :doc:`/reference/language/pous/function-block`: the ``FUNCTION_BLOCK`` unit
