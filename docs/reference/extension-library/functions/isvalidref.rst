============
__ISVALIDREF
============

Returns whether a ``REFERENCE TO`` variable is currently bound to a valid
target.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Not part of the standard (Beckhoff TwinCAT / CODESYS extension)
   * - **Support**
     - Supported (requires ``--allow-reference-to`` or ``--dialect twincat`` / ``--dialect codesys``)

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
     - ``REFERENCE TO ANY``
     - ``BOOL``
     - Supported

Description
-----------

``__ISVALIDREF`` reports whether a :doc:`REFERENCE TO
</reference/language/data-types/derived/reference-types>` variable currently
refers to a valid target. It returns ``TRUE`` once the reference has been bound
with ``REF=`` and ``FALSE`` while it is unbound (null). It is not part of the
IEC 61131-3 standard but is a Beckhoff TwinCAT / CODESYS extension, typically
used to guard a dereference so that reading or writing through an unbound
reference is avoided.

The argument is the reference itself — ``__ISVALIDREF`` inspects the binding,
so (unlike an ordinary read of a ``REFERENCE TO`` variable) the argument is
*not* automatically dereferenced. IronPLC lowers ``__ISVALIDREF(r)`` to the
equivalent comparison ``r <> NULL``.

``__ISVALIDREF`` is recognized as a builtin only when ``--allow-reference-to``
is enabled. Without that flag it is treated as an ordinary (undeclared)
function name.

Enabling
--------

``__ISVALIDREF`` is a vendor extension and must be explicitly enabled:

.. code-block:: shell

   ironplcc check --allow-reference-to main.st

Or use a dialect that enables ``REFERENCE TO`` support:

.. code-block:: shell

   ironplcc check --dialect twincat main.st

See :doc:`/explanation/enabling-dialects-and-features` for more information
about dialects and feature flags.

Example
-------

.. playground-with-program::
   :vars: x : INT; r : REFERENCE TO INT; valid : BOOL;
   :allows: reference-to

   valid := __ISVALIDREF(r);   (* valid = FALSE, r is unbound *)
   r REF= x;
   valid := __ISVALIDREF(r);   (* valid = TRUE, r is now bound *)

See Also
--------

- :doc:`/reference/language/data-types/derived/reference-types` — ``REFERENCE TO`` reference types
- :doc:`/explanation/enabling-dialects-and-features` — enabling vendor extensions
