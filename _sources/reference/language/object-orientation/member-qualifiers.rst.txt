=================
Member Qualifiers
=================

A member qualifier is a word between ``FUNCTION_BLOCK`` or ``METHOD`` and
the name that says who may use the declaration (``PUBLIC``, ``PRIVATE``,
``PROTECTED``, ``INTERNAL``) or how a derived type may treat it
(``FINAL``, ``OVERRIDE``, :doc:`ABSTRACT <abstract>`). Qualifiers are part
of the object-oriented programming introduced in IEC 61131-3 Edition 3, and
CODESYS and TwinCAT use the same words.

.. note::

   Except for ``ABSTRACT``, the qualifiers are never keywords. A word is
   taken as a qualifier only between ``FUNCTION_BLOCK`` or ``METHOD`` and
   the name, so ``Final`` or ``Override`` can still be a variable, method
   or function block name. A qualifier on a function block needs
   ``--allow-fb-inheritance`` and reports
   :doc:`P4062 </reference/compiler/problems/P4062>` without it. See
   :doc:`/explanation/enabling-dialects-and-features` for the flags and
   dialects reference.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Edition 3 (object-oriented programming)
   * - **Support**
     - Parsed and checked: a combination, order or position that TwinCAT
       rejects reports :doc:`P4063 </reference/compiler/problems/P4063>`.
       The qualifiers are not enforced otherwise. A ``PRIVATE`` method can
       be called from outside its function block, and a ``FINAL`` function
       block can be extended.

Syntax
------

.. code-block:: bnf

   FUNCTION_BLOCK [access_specifier] [FINAL | ABSTRACT] fb_name ...
   METHOD [access_specifier] [FINAL | ABSTRACT] [OVERRIDE] method_name [: return_type] ...

   access_specifier ::= PUBLIC | PRIVATE | PROTECTED | INTERNAL

The rules, checked against TwinCAT 3.1.4024:

.. include:: /includes/member-qualifier-rules.rst

Each rule that is broken reports
:doc:`P4063 </reference/compiler/problems/P4063>`.

Example
-------

.. code-block::

   FUNCTION_BLOCK PUBLIC ABSTRACT FB_Axis
       VAR
           position : REAL;
       END_VAR

       METHOD PUBLIC ABSTRACT MoveTo
           VAR_INPUT
               target : REAL;
           END_VAR
       END_METHOD

       METHOD PRIVATE Clamp
           IF position < 0.0 THEN
               position := 0.0;
           END_IF;
       END_METHOD
   END_FUNCTION_BLOCK

``MoveTo`` has no body; a function block that
:doc:`extends <extends>` ``FB_Axis`` provides it. ``Clamp`` is meant for
use inside ``FB_Axis`` only.

See Also
--------

- :doc:`abstract` — function block types that cannot be instantiated
- :doc:`method` — declare a method on a function block type
- :doc:`extends` — derive from a base type
- :doc:`P4062 </reference/compiler/problems/P4062>` — a function block
  qualifier without the flag
- :doc:`P4063 </reference/compiler/problems/P4063>` — an invalid
  qualifier combination, order or position
