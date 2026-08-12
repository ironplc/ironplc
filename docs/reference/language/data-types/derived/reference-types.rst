===============
Reference Types
===============

A reference type holds a pointer to a variable. References allow indirect
access — reading or writing through the reference affects the original
variable.

.. include:: ../../../../includes/requires-edition3.rst

.. tip::

   References can also be enabled without full Edition 3 by passing
   ``--allow-ref-to``, or by selecting a dialect that includes it.
   See :doc:`/explanation/enabling-dialects-and-features`.

.. note::

   Beckhoff TwinCAT and CODESYS spell references ``REFERENCE TO`` and bind them
   with the ``REF=`` operator (``r REF= x;``) rather than ``REF_TO`` and
   ``r := REF(x);``. Enable this variant with ``--allow-reference-to``, or a
   dialect that includes it. It describes the same underlying reference,
   but — unlike ``REF_TO`` — a ``REFERENCE TO`` variable *auto-dereferences*: a
   bare use reads through the reference and a bare ``:=`` writes through it, with
   no ``^`` needed. Only the binding operator ``REF=`` and ``__ISVALIDREF(r)``
   act on the reference itself:

   .. code-block::

      r : REFERENCE TO INT;
      r REF= counter;         (* bind the reference *)
      value := r;             (* read through the reference (implicit deref) *)
      r := 99;                (* write through the reference (implicit deref) *)
      valid := __ISVALIDREF(r);  (* TRUE once bound, FALSE while unbound *)

.. note::

   Beckhoff TwinCAT and CODESYS also have a pointer type, spelled
   ``POINTER TO`` and bound with the
   :doc:`ADR() </reference/extension-library/functions/adr>` address-of
   operator. Enable it with ``--allow-pointer-to`` and ``--allow-adr``, or a
   dialect that includes them. A ``POINTER TO`` variable behaves like
   ``REF_TO``: reading or writing the target requires the explicit
   dereference operator ``^``, unlike the auto-dereferencing
   ``REFERENCE TO``:

   .. code-block::

      p : POINTER TO INT;
      p := ADR(counter);      (* point at counter *)
      value := p^;            (* read through the pointer *)
      p^ := 99;               (* write through the pointer *)

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.3.3.1 (Edition 3)
   * - **Support**
     - Supported (:doc:`Edition 3 </reference/language/edition-support>`)

Syntax
------

Declare a reference variable with ``REF_TO``:

.. code-block:: bnf

   variable_name : REF_TO element_type

You can also create a named reference type:

.. code-block::

   TYPE
       IntRef : REF_TO INT;
   END_TYPE

Operators
---------

.. _ref-operator-ref:

``REF()``
   Creates a reference to a variable:

   .. code-block::

      r := REF(counter);

.. _ref-operator-deref:

``^`` (dereference)
   Reads or writes the referenced variable:

   .. code-block::

      value := r^;    (* read through reference *)
      r^ := 99;       (* write through reference *)

.. _ref-operator-null:

``NULL``
   A literal representing an empty reference. Can be assigned to any
   ``REF_TO`` variable and compared with ``=`` or ``<>``:

   .. code-block::

      r := NULL;
      IF r <> NULL THEN
          value := r^;
      END_IF;

Example
-------

.. playground::
   :dialect: iec61131-3-ed3

   PROGRAM main
     VAR
       counter : INT := 42;
       r : REF_TO INT := REF(counter);
       value : INT;
     END_VAR

     (* Read through the reference *)
     value := r^;

     (* Write through the reference — changes counter *)
     r^ := 99;
   END_PROGRAM

Restrictions
------------

- ``REF()`` accepts only simple named variables (not array elements or
  literals).
- References to temporary variables (``VAR_TEMP``, function parameters)
  are not allowed.
- Nested references (``REF_TO REF_TO``) are not supported.
- Arithmetic on references is not supported by default. Use
  ``--allow-ref-arithmetic`` to enable it.
- Only ``=`` and ``<>`` comparison operators work with references by default.
  Use ``--allow-ref-arithmetic`` to enable ordering comparisons.

Related Problem Codes
---------------------

- :doc:`/reference/compiler/problems/P2028` — REF() operand must be a simple variable
- :doc:`/reference/compiler/problems/P2031` — Dereference requires a REF_TO type
- :doc:`/reference/compiler/problems/P2032` — Reference type mismatch
- :doc:`/reference/compiler/problems/P2034` — NULL can only be assigned to REF_TO types

External References
-------------------

- `Beckhoff TwinCAT: POINTER <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2529453451.html>`__
- `Beckhoff TwinCAT: REFERENCE <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2529458827.html>`__
- `CODESYS: Data Type POINTER TO <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_datatype_pointer.html>`__
- `Schneider Machine Expert: Pointers <https://product-help.schneider-electric.com/Machine%20Expert/V1.1/en/SoMProg/SoMProg/Data_Types/Data_Types-8.htm>`__
- `ABB: Pointers <https://help.plc.abb.com/AB270_en/_cds_datatype_pointer.html>`__

See Also
--------

- :doc:`/reference/extension-library/functions/adr` — ``ADR``, the TwinCAT/CODESYS address-of operator
- :doc:`/reference/extension-library/functions/isvalidref` — ``__ISVALIDREF``, the TwinCAT/CODESYS reference-validity builtin
- :doc:`/reference/language/edition-support` — edition flags
- :doc:`/reference/language/variables/scope` — variable scope keywords
