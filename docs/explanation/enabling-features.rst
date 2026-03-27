=================
Enabling Features
=================

IronPLC aims to let you take code from another PLC environment and use it
without changes. To support this, IronPLC uses **dialects** — named presets
that select the IEC 61131-3 edition and a default set of vendor extensions.
Individual ``--allow-*`` flags provide fine-grained control on top of the
selected dialect.

---------------------------------
Supported Dialects
---------------------------------

**iec61131-3-ed2** *(default)*
   Strict IEC 61131-3:2003 (Edition 2). No vendor extensions are enabled.
   This is the default when no dialect is specified.

**iec61131-3-ed3**
   Strict IEC 61131-3:2013 (Edition 3). Enables Edition 3 keywords
   including :doc:`LTIME </reference/language/data-types/ltime>`,
   :doc:`LDATE </reference/language/data-types/ldate>`,
   :doc:`LTIME_OF_DAY </reference/language/data-types/ltime-of-day>`,
   :doc:`LDATE_AND_TIME </reference/language/data-types/ldate-and-time>`,
   ``REF_TO``, ``REF``, and ``NULL``. No vendor extensions.

**rusty**
   RuSTy-compatible dialect. Uses Edition 2 as a base (so Edition 3 type
   names like ``LDT`` remain available as identifiers) and enables
   ``REF_TO`` support plus all vendor extensions.

Editions are additive — enabling a later edition includes all features from
earlier editions.

See :doc:`/reference/language/edition-support` for a complete list of
features that require a specific edition.

.. tip::

   Run ``ironplcc dialects`` to see which features each dialect enables.

---------------------------------
How to Select a Dialect
---------------------------------

Command Line
^^^^^^^^^^^^

Pass the ``--dialect`` flag when running :program:`ironplcc`:

.. code-block:: shell

   ironplcc check --dialect rusty main.st

.. code-block:: shell

   ironplcc check --dialect iec61131-3-ed3 main.st

See :doc:`/reference/compiler/ironplcc` for all compiler options.

Visual Studio Code
^^^^^^^^^^^^^^^^^^

Set the :code:`ironplc.dialect` setting:

1. Open :menuselection:`File --> Preferences --> Settings`
   (or :menuselection:`Code --> Preferences --> Settings` on macOS).
2. Search for ``ironplc``.
3. Change :guilabel:`Dialect` to the desired value
   (e.g., ``rusty`` or ``iec61131-3-ed3``).

Or add it directly to your :file:`settings.json`:

.. code-block:: json

   {
     "ironplc.dialect": "rusty"
   }

See :doc:`/reference/editor/settings` for all extension settings.

---------------------------------
Enabling Specific Features
---------------------------------

Individual ``--allow-*`` flags can be combined with any dialect to enable
additional features on top of the dialect's defaults. Flags can only enable
features — they never disable features that a dialect already includes.

``--allow-c-style-comments``
   Allow C-style comments (``//`` line comments and ``/* */`` block comments).
   These are not part of the IEC 61131-3 standard but are supported by many
   PLC environments.

``--allow-missing-semicolon``
   Allow missing semicolons after keyword statements like ``END_IF`` and
   ``END_STRUCT``.

``--allow-top-level-var-global``
   Allow :code:`VAR_GLOBAL` declarations at the top level of a file,
   outside of a :code:`CONFIGURATION` block. See
   :doc:`/reference/language/variables/scope`.

``--allow-constant-type-params``
   Allow constant references in type parameters such as array bounds and
   string lengths (e.g., ``ARRAY[1..MY_CONST] OF INT`` or
   ``STRING[MY_CONST]``). See :doc:`/reference/language/data-types/array-types`.

``--allow-empty-var-blocks``
   Allow empty variable blocks (``VAR END_VAR``, ``VAR_INPUT END_VAR``, etc.).
   Some PLC environments permit variable blocks with no declarations.

``--allow-time-as-function-name``
   Allow ``TIME`` to be used as a function name (e.g., ``TIME()``).
   Required for OSCAT compatibility where ``TIME()`` reads the PLC system
   clock.

``--allow-ref-to``
   Allow ``REF_TO``, ``REF()``, and ``NULL`` syntax without enabling full
   Edition 3. This is useful for libraries that use references but also use
   Edition 3 type names (like ``LDT`` or ``LTIME``) as identifiers.

Pass the flag when running :program:`ironplcc`:

.. code-block:: shell

   ironplcc check --allow-c-style-comments --allow-empty-var-blocks main.st

Or combine with a dialect:

.. code-block:: shell

   ironplcc check --dialect iec61131-3-ed3 --allow-c-style-comments main.st

See :doc:`/reference/compiler/ironplcc` for all compiler options.
