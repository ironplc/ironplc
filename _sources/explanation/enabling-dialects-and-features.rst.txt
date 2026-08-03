==============================
Enabling Dialects and Features
==============================

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

   **Enables:** nothing beyond strict IEC 61131-3 (no vendor extensions).

**iec61131-3-ed3**
   Strict IEC 61131-3:2013 (Edition 3). Enables Edition 3 keywords
   including :doc:`LTIME </reference/language/data-types/elementary/ltime>`,
   :doc:`LDATE </reference/language/data-types/elementary/ldate>`,
   :doc:`LTIME_OF_DAY </reference/language/data-types/elementary/ltime-of-day>`,
   :doc:`LDATE_AND_TIME </reference/language/data-types/elementary/ldate-and-time>`,
   :doc:`REF_TO </reference/language/data-types/derived/reference-types>`,
   :doc:`REF </reference/language/data-types/derived/reference-types>`, and
   :doc:`NULL </reference/language/data-types/derived/reference-types>`. No vendor extensions.

   **Enables:** Edition 3 keywords, plus ``--allow-partial-access-syntax``.

**rusty**
   RuSTy-compatible dialect. Uses Edition 2 as a base (so Edition 3 type
   names like :doc:`LDT </reference/language/data-types/elementary/ldate-and-time>` remain available as identifiers) and enables
   :doc:`REF_TO </reference/language/data-types/derived/reference-types>` support plus all vendor extensions.

   **Enables:** ``--allow-c-style-comments``, ``--allow-missing-semicolon``,
   ``--allow-top-level-var-global``, ``--allow-constant-type-params``,
   ``--allow-empty-var-blocks``, ``--allow-time-as-function-name``,
   ``--allow-ref-to``, ``--allow-ref-arithmetic``,
   ``--allow-ref-stack-variables``, ``--allow-ref-type-punning``,
   ``--allow-int-to-bool-initializer``, ``--allow-sizeof``,
   ``--allow-system-uptime-global``, ``--allow-cross-family-widening``,
   ``--allow-partial-access-syntax``, ``--allow-pragmas``,
   ``--allow-short-circuit-operators``,
   ``--allow-mixed-located-var-declarations``,
   ``--allow-constant-initializer-expressions``,
   ``--allow-bit-string-case-labels``, ``--allow-paren-string-length``,
   ``--allow-struct-initializer-expressions``, and
   ``--allow-fb-inheritance``.

**codesys**
   CODESYS-compatible dialect. Uses Edition 2 as a base (so identifiers like
   :doc:`LDT </reference/language/data-types/elementary/ldate-and-time>` are
   preserved) and enables
   :doc:`REF_TO </reference/language/data-types/derived/reference-types>`
   together with the vendor extensions that the CODESYS IDE accepts. The
   implicit :doc:`__SYSTEM_UP_TIME </reference/extension-library/variables/system-uptime>`
   globals are not pre-bound under this dialect, since they are an IronPLC
   runtime convention rather than a CODESYS feature.

   **Enables:** ``--allow-c-style-comments``, ``--allow-missing-semicolon``,
   ``--allow-top-level-var-global``, ``--allow-constant-type-params``,
   ``--allow-empty-var-blocks``, ``--allow-time-as-function-name``,
   ``--allow-ref-to``, ``--allow-reference-to``, ``--allow-ref-arithmetic``,
   ``--allow-ref-stack-variables``, ``--allow-ref-type-punning``,
   ``--allow-int-to-bool-initializer``, ``--allow-sizeof``,
   ``--allow-cross-family-widening``, ``--allow-partial-access-syntax``,
   ``--allow-pragmas``, ``--allow-short-circuit-operators``,
   ``--allow-mixed-located-var-declarations``,
   ``--allow-constant-initializer-expressions``,
   ``--allow-bit-string-case-labels``, ``--allow-paren-string-length``,
   ``--allow-struct-initializer-expressions``, and
   ``--allow-fb-inheritance``.

**twincat**
   Beckhoff TwinCAT-compatible dialect. TwinCAT 3 is built on the CODESYS V3
   runtime, so it uses an Edition 2 base (identifiers like
   :doc:`LDT </reference/language/data-types/elementary/ldate-and-time>` are
   preserved) and enables the vendor extensions TwinCAT shares with CODESYS,
   such as curly-brace pragmas, C-style comments, and the ``AND_THEN``
   short-circuit operator. Unlike ``codesys``, it does **not** enable the
   ``REF_TO`` / ``REF()`` / ``NULL`` reference extensions: TwinCAT spells
   references ``REFERENCE TO`` (bound with ``REF=``), which this dialect
   enables instead via ``--allow-reference-to``. (Pointer types —
   ``POINTER TO`` with ``ADR()`` — are not parsed yet.) As with ``codesys``,
   the implicit
   :doc:`__SYSTEM_UP_TIME </reference/extension-library/variables/system-uptime>`
   globals are not pre-bound, since they are an IronPLC runtime convention
   rather than a TwinCAT feature.

   **Enables:** ``--allow-c-style-comments``, ``--allow-missing-semicolon``,
   ``--allow-top-level-var-global``, ``--allow-constant-type-params``,
   ``--allow-empty-var-blocks``, ``--allow-time-as-function-name``,
   ``--allow-reference-to``, ``--allow-int-to-bool-initializer``,
   ``--allow-sizeof``, ``--allow-cross-family-widening``,
   ``--allow-partial-access-syntax``, ``--allow-pragmas``,
   ``--allow-short-circuit-operators``,
   ``--allow-mixed-located-var-declarations``,
   ``--allow-bit-string-case-labels``, ``--allow-paren-string-length``, and
   ``--allow-struct-initializer-expressions``.

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
features — they never disable features that a dialect already includes. To see
which flags a dialect already enables by default, see `Supported Dialects`_.

``--allow-c-style-comments``
   Allow C-style comments (``//`` line comments and ``/* */`` block comments).
   These are not part of the IEC 61131-3 standard but are supported by many
   PLC environments.

``--allow-missing-semicolon``
   Allow missing semicolons after keyword statements like ``END_IF`` and
   ``END_STRUCT``. Also allows a ``CASE`` branch with no statements at
   all (a label that falls straight through to the next label, ``ELSE``,
   or ``END_CASE``) -- strict IEC 61131-3 only allows this via an
   explicit empty statement (``5: ;``); this fills in the dropped ``;``.

``--allow-top-level-var-global``
   Allow :code:`VAR_GLOBAL` declarations at the top level of a file,
   outside of a :code:`CONFIGURATION` block. See
   :doc:`/reference/language/variables/scope`.

``--allow-constant-type-params``
   Allow constant references in type parameters such as array bounds and
   string lengths (e.g., ``ARRAY[1..MY_CONST] OF INT`` or
   ``STRING[MY_CONST]``). See :doc:`/reference/language/data-types/derived/array-types`.

``--allow-empty-var-blocks``
   Allow empty variable blocks (``VAR END_VAR``, ``VAR_INPUT END_VAR``, etc.).
   Some PLC environments permit variable blocks with no declarations.

``--allow-time-as-function-name``
   Allow ``TIME`` to be used as a function name (e.g., ``TIME()``).
   Required for OSCAT compatibility where ``TIME()`` reads the PLC system
   clock.

``--allow-ref-to``
   Allow ``REF_TO``, ``REF()``, and ``NULL`` syntax without enabling full
   Edition 3. This is useful when you need references but want to keep
   Edition 2 keyword handling for the rest of your code. See
   :doc:`/reference/language/data-types/derived/reference-types`.

``--allow-reference-to``
   Allow the Beckhoff TwinCAT / CODESYS ``REFERENCE TO`` reference type and the
   ``REF=`` binding operator. This is the TwinCAT/CODESYS-facing alternative to
   ``--allow-ref-to``: the two describe the same underlying reference but with
   different surface syntax. The compiler does not restrict flag combinations, so
   ``--allow-ref-to`` and ``--allow-reference-to`` may be set at once. See
   :doc:`/reference/language/data-types/derived/reference-types`.

``--allow-ref-arithmetic``
   Allow arithmetic (``+``, ``-``) and ordering comparisons (``<``, ``>``,
   ``<=``, ``>=``) on ``REF_TO`` types. By default, only ``=`` and ``<>``
   are permitted on references.

``--allow-ref-stack-variables``
   Allow ``REF()`` on stack-allocated variables (``VAR_TEMP`` and function
   ``VAR_INPUT``/``VAR_OUTPUT``). Required for OSCAT patterns where the
   reference does not escape the call.

``--allow-ref-type-punning``
   Allow assigning between ``REF_TO`` types of different base types (type
   punning), such as reinterpreting the bits of a ``REAL`` through a
   ``REF_TO DWORD``.

``--allow-int-to-bool-initializer``
   Allow integer literals ``0`` and ``1`` as ``BOOL`` variable initializers
   (e.g., ``debug : BOOL := 0;``). The compiler rewrites ``0`` to ``FALSE``
   and ``1`` to ``TRUE``. This is a universal vendor extension supported by
   CoDeSys, TwinCAT, RuSTy, and virtually every PLC runtime.

``--allow-sizeof``
   Allow the ``SIZEOF()`` operator that returns the size in bytes of a
   variable or type. This is a vendor extension supported by CODESYS,
   TwinCAT, and RuSTy. See
   :doc:`/reference/extension-library/functions/sizeof`.

``--allow-system-uptime-global``
   Expose ``__SYSTEM_UP_TIME`` (``TIME``) and ``__SYSTEM_UP_LTIME``
   (``LTIME``) as implicit ``VAR_GLOBAL`` values holding the VM's monotonic
   uptime. This is an IronPLC runtime convention.

``--allow-cross-family-widening``
   Allow implicit widening between bit-string and integer type families.
   For example, passing a ``BYTE`` variable where an ``INT`` parameter is
   expected, or passing a bare integer literal ``0`` where a ``BYTE``
   parameter is expected. This is a vendor extension supported by CODESYS,
   TwinCAT, and RuSTy.

``--allow-partial-access-syntax``
   Allow IEC 61131-3:2013 partial-access bit syntax ``.%Xn`` (e.g.,
   ``myByte.%X3`` to access bit 3 of a ``BYTE``). Semantically equivalent to
   the short form ``.n``. Byte/word/dword/lword partial access (``.%Bn``,
   ``.%Wn``, ``.%Dn``, ``.%Ln``) is not yet supported.

``--allow-pragmas``
   Allow curly-brace pragmas such as ``{attribute 'qualified_only'}`` and
   ``{attribute 'strict'}``. These are CODESYS-core syntax (documented by
   CODESYS itself, and inherited by any IDE built on the CODESYS V3 runtime,
   including Beckhoff TwinCAT and Schneider Electric Machine Expert). A
   pragma is parsed and discarded like a comment — its contents are not yet
   interpreted. Pragmas do not nest; an unclosed ``{`` still produces a parse
   error.

``--allow-short-circuit-operators``
   Allow the ``AND_THEN`` short-circuit boolean operator, a
   Beckhoff/CODESYS extension. Unlike plain ``AND`` (which always
   evaluates both operands), ``AND_THEN`` only evaluates its right
   operand when the left operand is ``TRUE`` — commonly used to guard a
   dereference (``ptr <> 0 AND_THEN ptr^ = 99``). ``ironplcc check``
   fully supports ``AND_THEN`` (parsing, type-checking, and
   round-tripping through plc2plc with its spelling preserved — it is
   *not* normalized to ``AND``, since the short-circuit behavior is a
   real, externally-visible difference in TwinCAT/CODESYS). Codegen
   (``ironplcc compile``) does not yet implement the short-circuit
   evaluation this operator requires and refuses to compile it
   (problem :doc:`P9999 </reference/compiler/problems/P9999>`) rather
   than silently emitting eager (behaviorally incorrect) bytecode.

``--allow-mixed-located-var-declarations``
   Allow an ``AT``-located variable (complete address like ``AT %IX0.0``,
   or incomplete/wildcard address like ``AT %I*``) inside an otherwise
   plain ``VAR``/``VAR_INPUT``/``VAR_OUTPUT`` block, instead of requiring
   located variables to live in their own dedicated block. The IEC 61131-3
   standard requires located and plain variables to be declared in
   separate blocks; real CODESYS/TwinCAT code commonly mixes them. Without
   this flag, mixing produces problem
   :doc:`P4036 </reference/compiler/problems/P4036>`. A block containing
   *only* located variables is unaffected by this flag — it is standard
   syntax and always allowed.

``--allow-constant-initializer-expressions``
   Allow a ``VAR`` initializer to be a constant *expression* — arithmetic
   between literals and/or references to declared ``CONSTANT`` variables
   (e.g. ``scaled : LREAL := SCALE*4.0;``) — rather than only a bare
   literal. The IEC 61131-3 standard's initializer grammar permits only
   literals in this position; this vendor extension folds the expression
   to a literal at compile time. Using this form without the flag produces
   :doc:`P4037 </reference/compiler/problems/P4037>`; if the expression
   does not fully reduce to a constant (e.g. it references a
   non-``CONSTANT`` variable), it produces
   :doc:`P4038 </reference/compiler/problems/P4038>`.

``--allow-bit-string-case-labels``
   Allow a hex, binary, or octal bit-string literal (e.g. ``16#D012``,
   ``2#1010``, ``8#17``) as a ``CASE`` label. The IEC 61131-3 standard
   grammar for a case label permits only a subrange, a *decimal*
   ``signed_integer``, or an enumerated value; radix-prefixed literals are
   separate productions the standard does not include here. Real
   TwinCAT/CODESYS code uses them. Without this flag, such a label produces
   :doc:`P4041 </reference/compiler/problems/P4041>`. A plain decimal label
   (``5:``) is standard syntax and is always allowed.

``--allow-paren-string-length``
   Allow a string type's maximum length to be delimited with parentheses
   (``STRING(255)``, ``WSTRING(100)``) in addition to the standard square
   brackets (``STRING[255]``). The IEC 61131-3 standard grammar declares a
   string length only with brackets; the parenthesis form is a vendor
   extension. Without this flag, the parenthesis form produces
   :doc:`P4042 </reference/compiler/problems/P4042>`. The bracket form is
   standard syntax and is always allowed, and the delimiters must match
   (``STRING[255)`` is always a syntax error). The renderer normalizes the
   parenthesis form to brackets.

``--allow-struct-initializer-expressions``
   Allow a general (non-constant) expression — such as a pointer
   dereference plus member access (``pDevice^.Delta``) — as the value in a
   structured or call-style initializer's ``name := value`` pairs (e.g.
   ``tonDelta : TON := (PT := pDevice^.Delta);``). The IEC 61131-3 standard
   grammar for a structured initializer value permits only a constant,
   enumerated value, array initializer, or nested structure initializer;
   a value computed at instantiation time is a vendor extension used by
   TwinCAT/CODESYS. Without this flag, such a value produces
   :doc:`P4043 </reference/compiler/problems/P4043>`. A constant value is
   standard syntax and is always allowed.

``--allow-fb-inheritance``
   Allow function-block inheritance syntax: ``EXTENDS``/
   ``IMPLEMENTS``/``ABSTRACT`` on ``FUNCTION_BLOCK`` declarations, and
   ``INTERFACE`` declarations. These are parsed and the
   ``EXTENDS``/``IMPLEMENTS``/``INTERFACE`` names are recognized as valid
   types, but inheritance, interface dispatch, method/property
   declarations, and abstract-instantiation checking are not yet
   semantically supported — using them produces problem
   :doc:`P9004 </reference/compiler/problems/P9004>` rather than a parse
   error. Enabled by ``--dialect=rusty`` and ``--dialect=codesys``.

Pass the flag when running :program:`ironplcc`:

.. code-block:: shell

   ironplcc check --allow-c-style-comments --allow-empty-var-blocks main.st

Or combine with a dialect:

.. code-block:: shell

   ironplcc check --dialect iec61131-3-ed3 --allow-c-style-comments main.st

See :doc:`/reference/compiler/ironplcc` for all compiler options.
