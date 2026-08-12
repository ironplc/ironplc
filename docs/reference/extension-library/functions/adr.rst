===
ADR
===

Returns the address of a variable as a typed pointer.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Not part of the standard (Beckhoff TwinCAT / CODESYS extension)
   * - **Support**
     - Supported (requires ``--allow-adr``)

Signatures
----------

.. list-table::
   :header-rows: 1
   :widths: 10 30 30 30

   * - #
     - Input (IN)
     - Return Type
     - Support
   * - 1
     - ``ANY`` (a named variable)
     - ``POINTER TO`` *type of input*
     - Supported

Description
-----------

``ADR`` returns the address of a variable, typically for assignment to a
:doc:`POINTER TO </reference/language/data-types/derived/reference-types>`
variable that is later dereferenced with ``^``. It is not part of the
IEC 61131-3 standard but is a Beckhoff TwinCAT / CODESYS extension:

.. code-block::

   pNumber := ADR(iNumber1);   (* point at iNumber1 *)
   iNumber2 := pNumber^;       (* read through the pointer *)
   pNumber^ := 7;              (* write through the pointer *)

In TwinCAT and CODESYS, ``ADR`` yields a raw machine address (``PVOID``)
that those environments also allow assigning to integer types such as
``DWORD`` or ``LWORD``. IronPLC variables do not have byte addresses, so
``ADR(x)`` instead has the typed result ``POINTER TO`` *type of x* and is
type-checked against the destination pointer's target type. This is
stricter than TwinCAT and CODESYS — see Restrictions below for the
address-manipulation patterns that do not carry over.

Enabling
--------

``ADR`` is a language extension and must be explicitly enabled:

.. code-block:: shell

   ironplcc check --allow-pointer-to --allow-adr main.st

.. |flag| replace:: ``--allow-adr``
.. include:: /includes/enabled-by-flag.rst

Example
-------

.. playground-with-program::
   :vars: counter : INT := 42; p : POINTER TO INT; value : INT;
   :allows: pointer-to,adr

   p := ADR(counter);
   value := p^;     (* value = 42 *)
   p^ := 99;        (* counter = 99 *)

Restrictions
------------

- The operand must be a simple named variable. Addresses of sub-objects —
  array elements (``ADR(arr[i])``), structure fields (``ADR(s.field)``) —
  and of literals or call results are rejected.
- The operand must not be a stack-allocated variable (``VAR_TEMP``, function
  parameters), which would produce a dangling pointer.
- The result must be assigned to a pointer or reference variable. Assigning
  an address to an integer variable (``DWORD``/``LWORD``), pointer
  arithmetic, and byte-level tricks such as ``MEMCPY`` do not carry over
  from TwinCAT / CODESYS.

Related Problem Codes
---------------------

- :doc:`/reference/compiler/problems/P2028` — operand must be a simple variable
- :doc:`/reference/compiler/problems/P2029` — operand is a stack-allocated variable
- :doc:`/reference/compiler/problems/P2030` — operand is an array element
- :doc:`/reference/compiler/problems/P2032` — pointer target type mismatch

External References
-------------------

- `Beckhoff TwinCAT: ADR <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2529015179.html>`__
- `CODESYS: Operator ADR <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_adr.html>`__
- `ABB: Operator ADR <https://help.plc.abb.com/_cds_operator_ADR.html>`__

See Also
--------

- :doc:`/reference/language/data-types/derived/reference-types` — ``POINTER TO``, ``REF_TO``, and ``REFERENCE TO``
- :doc:`/explanation/enabling-dialects-and-features` — enabling language extensions
