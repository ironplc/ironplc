=================
Enabling Features
=================

IronPLC aims to let you take code from another PLC environment and use it
without changes. However, the IEC 61131-3 standard has evolved through
multiple editions, and some features require you to tell IronPLC which
edition your code targets.

---------------------------------
Editions of the Standard
---------------------------------

The IEC 61131-3 standard has been published in several editions:

**Edition 2 (2003)**
   The widely deployed baseline. IronPLC uses this edition by default.

**Edition 3 (2013)**
   Adds new data types (:doc:`LTIME </reference/language/data-types/ltime>`,
   :doc:`LDATE </reference/language/data-types/ldate>`,
   :doc:`LTIME_OF_DAY </reference/language/data-types/ltime-of-day>`,
   :doc:`LDATE_AND_TIME </reference/language/data-types/ldate-and-time>`)
   and other language enhancements.

Editions are additive — enabling a later edition includes all features from
earlier editions.

See :doc:`/reference/language/edition-support` for a complete list of
features that require a specific edition.

---------------------------------
How to Enable an Edition
---------------------------------

Command Line
^^^^^^^^^^^^

Pass the ``--std-iec-61131-3`` flag when running :program:`ironplcc`:

.. code-block:: shell

   ironplcc check --std-iec-61131-3=2013 main.st

See :doc:`/reference/compiler/ironplcc` for all compiler options.

Visual Studio Code
^^^^^^^^^^^^^^^^^^

Set the :code:`ironplc.std61131Version` setting to :code:`2013`:

1. Open :menuselection:`File --> Preferences --> Settings`
   (or :menuselection:`Code --> Preferences --> Settings` on macOS).
2. Search for ``ironplc``.
3. Change :guilabel:`Std 61131 Version` to ``2013``.

Or add it directly to your :file:`settings.json`:

.. code-block:: json

   {
     "ironplc.std61131Version": "2013"
   }

See :doc:`/reference/editor/settings` for all extension settings.

---------------------------------
Vendor Extensions
---------------------------------

Some PLC vendors support features beyond the IEC 61131-3 standard. IronPLC
provides flags for these common vendor extensions to improve compatibility
with code written for other PLC environments.

``--allow-all``
   Enable all vendor extensions at once (except ``--allow-missing-semicolon``,
   which must be passed explicitly).

``--allow-top-level-var-global``
   Allow :code:`VAR_GLOBAL` declarations at the top level of a file,
   outside of a :code:`CONFIGURATION` block. See
   :doc:`/reference/language/variables/scope`.

``--allow-constant-type-params``
   Allow constant references in type parameters such as array bounds and
   string lengths (e.g., ``ARRAY[1..MY_CONST] OF INT`` or
   ``STRING[MY_CONST]``). See :doc:`/reference/language/data-types/array-types`.

``--allow-c-style-comments``
   Allow C-style comments (``//`` line comments and ``/* */`` block comments).
   These are not part of the IEC 61131-3 standard but are supported by many
   PLC environments.

``--allow-empty-var-blocks``
   Allow empty variable blocks (``VAR END_VAR``, ``VAR_INPUT END_VAR``, etc.).
   Some PLC environments permit variable blocks with no declarations.

``--allow-time-as-function-name``
   Allow ``TIME`` to be used as a function name (e.g., ``TIME()``).
   Required for OSCAT compatibility where ``TIME()`` reads the PLC system
   clock.

``--allow-missing-semicolon``
   Allow missing semicolons after keyword statements like ``END_IF`` and
   ``END_STRUCT``. Note: this flag is **not** included in ``--allow-all``
   and must be passed explicitly.

Pass the flag when running :program:`ironplcc`:

.. code-block:: shell

   ironplcc check --allow-all main.st

Or enable individual extensions:

.. code-block:: shell

   ironplcc check --allow-top-level-var-global --allow-constant-type-params main.st

See :doc:`/reference/compiler/ironplcc` for all compiler options.

