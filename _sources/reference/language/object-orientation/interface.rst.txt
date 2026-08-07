=========
INTERFACE
=========

``INTERFACE`` declares an interface: a named set of method signatures with
no implementation. A function block type that :doc:`implements <implements>`
the interface promises to supply a body for each of those methods, which lets
instances of unrelated types be used through the same interface. An interface
declaration is terminated by ``END_INTERFACE``. Interfaces are part of the
object-oriented programming introduced in IEC 61131-3 Edition 3.

.. |keyword| replace:: ``INTERFACE``
.. |flag| replace:: ``--allow-fb-inheritance``
.. include:: /includes/oop-keyword-flag.rst

.. note::

   ``END_INTERFACE`` is the closing keyword of an interface declaration and
   is gated by the same flag. Like ``INTERFACE``, it is an ordinary
   identifier when the flag is not enabled.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Edition 3 (object-oriented programming)
   * - **Support**
     - Parsed only — not yet analyzed or executed
       (:doc:`P9999 </reference/compiler/problems/P9999>`). Enable with
       ``--allow-fb-inheritance``; see
       :doc:`/explanation/enabling-dialects-and-features`.

Syntax
------

.. code-block:: bnf

   INTERFACE interface_name [EXTENDS base_interface {, base_interface}]
       method_prototypes
   END_INTERFACE

An interface may :doc:`extend <extends>` one or more base interfaces,
inheriting their method signatures.

Example
-------

.. code-block::

   INTERFACE I_Drivable
   END_INTERFACE

   INTERFACE I_PoweredDrivable EXTENDS I_Drivable
   END_INTERFACE

   FUNCTION_BLOCK FB_Motor IMPLEMENTS I_Drivable
       VAR
           running : BOOL;
       END_VAR
   END_FUNCTION_BLOCK

.. note::

   Method declarations (``METHOD`` … ``END_METHOD``) inside an interface are
   not yet parsed, so interface bodies are currently empty. The interface
   name and any ``EXTENDS`` clause are recognized.

See Also
--------

- :doc:`implements` — provide the methods declared by an interface
- :doc:`extends` — derive an interface or function block from a base
- :doc:`abstract` — mark a function block type as not directly instantiable
- :doc:`/explanation/object-orientation` — inheritance, interfaces, and
  abstract types explained
