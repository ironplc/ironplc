==============================
Enabling Dialects and Features
==============================

IronPLC lets you take code from another PLC environment and use it
without changes. To support this, IronPLC uses **dialects** — named presets
that select the IEC 61131-3 edition, a default set of extensions, and the
behavior policies the platform documents. Individual ``--allow-*`` flags and
``--policy-*`` selections provide fine-grained control on top of the
selected dialect.

---------------------------------
Supported Dialects
---------------------------------

**iec61131-3-ed2** *(default)*
   Strict IEC 61131-3:2003 (Edition 2). No extensions are enabled.
   This is the default when no dialect is specified.

   **Enables:** nothing beyond strict IEC 61131-3 (no extensions).

   **Selects:** the default of every behavior policy
   (``--policy-string-to-num-non-numeric reject``,
   ``--policy-string-to-num-failure trap``).

**iec61131-3-ed3**
   Strict IEC 61131-3:2013 (Edition 3). Enables Edition 3 keywords
   including :doc:`LTIME </reference/language/data-types/elementary/ltime>`,
   :doc:`LDATE </reference/language/data-types/elementary/ldate>`,
   :doc:`LTIME_OF_DAY </reference/language/data-types/elementary/ltime-of-day>`,
   :doc:`LDATE_AND_TIME </reference/language/data-types/elementary/ldate-and-time>`,
   :doc:`REF_TO </reference/language/data-types/derived/reference-types>`,
   :doc:`REF </reference/language/data-types/derived/reference-types>`,
   :doc:`NULL </reference/language/data-types/derived/reference-types>`, and the
   :doc:`object-oriented keywords </reference/language/object-orientation/index>`
   (``EXTENDS``, ``IMPLEMENTS``, ``ABSTRACT``, ``INTERFACE``, ``METHOD``,
   ``PROPERTY``, ``THIS``, and ``SUPER``). No extensions.

   **Enables:** ``--allow-long-time-types``, ``--allow-ref-to`` (the
   Edition 3 keywords), ``--allow-partial-access-syntax``,
   ``--allow-fb-inheritance``, ``--allow-enum-explicit-values``, and
   ``--allow-continue``.

   **Selects:** the default of every behavior policy
   (``--policy-string-to-num-non-numeric reject``,
   ``--policy-string-to-num-failure trap``).

**rusty**
   RuSTy-compatible dialect. Uses Edition 2 as a base (so Edition 3 type
   names like :doc:`LDT </reference/language/data-types/elementary/ldate-and-time>` remain available as identifiers) and enables
   :doc:`REF_TO </reference/language/data-types/derived/reference-types>` support together with the extensions that RuSTy accepts (listed below).

   **Enables:** ``--allow-c-style-comments``, ``--allow-missing-semicolon``,
   ``--allow-top-level-var-global``, ``--allow-constant-type-params``,
   ``--allow-empty-var-blocks``, ``--allow-time-as-function-name``,
   ``--allow-ref-to``, ``--allow-ref-arithmetic``,
   ``--allow-ref-stack-variables``, ``--allow-ref-type-punning``,
   ``--allow-int-to-bool-initializer``, ``--allow-sizeof``,
   ``--allow-system-uptime-global``, ``--allow-cross-family-widening``,
   ``--allow-cross-family-conversion``, ``--allow-int-literal-to-bit-string``,
   ``--allow-bit-string-arithmetic``,
   ``--allow-partial-access-syntax``, ``--allow-pragmas``,
   ``--allow-short-circuit-operators``,
   ``--allow-mixed-located-var-declarations``,
   ``--allow-constant-initializer-expressions``,
   ``--allow-radix-case-labels``, ``--allow-paren-string-length``,
   ``--allow-struct-initializer-expressions``,
   ``--allow-fb-inheritance``, ``--allow-enum-explicit-values``,
   ``--allow-enum-base-type``, ``--allow-continue``,
   ``--allow-keywords-as-names``, and ``--allow-radix-subrange-bounds``.

   **Selects:** ``--policy-string-to-num-non-numeric reject`` and
   ``--policy-string-to-num-failure zero`` — RuSTy rejects a string with
   trailing characters and never faults.

**codesys**
   CODESYS-compatible dialect. Uses Edition 2 as a base and enables the
   long-time-type keywords
   (:doc:`LTIME </reference/language/data-types/elementary/ltime>`,
   :doc:`LDT </reference/language/data-types/elementary/ldate-and-time>`, etc.)
   and :doc:`REF_TO </reference/language/data-types/derived/reference-types>`
   together with the extensions that the CODESYS IDE accepts. The
   implicit :doc:`__SYSTEM_UP_TIME </reference/extension-library/variables/system-uptime>`
   globals are not pre-bound under this dialect, since they are an IronPLC
   runtime convention rather than a CODESYS feature.

   **Enables:** ``--allow-c-style-comments``, ``--allow-missing-semicolon``,
   ``--allow-top-level-var-global``, ``--allow-constant-type-params``,
   ``--allow-empty-var-blocks``, ``--allow-time-as-function-name``,
   ``--allow-long-time-types``,
   ``--allow-ref-to``, ``--allow-reference-to``, ``--allow-pointer-to``,
   ``--allow-adr``, ``--allow-persistent-var``,
   ``--allow-ref-arithmetic``,
   ``--allow-ref-stack-variables``, ``--allow-ref-type-punning``,
   ``--allow-int-to-bool-initializer``, ``--allow-sizeof``,
   ``--allow-cross-family-widening``,
   ``--allow-cross-family-conversion``, ``--allow-int-literal-to-bit-string``,
   ``--allow-bit-string-arithmetic``, ``--allow-partial-access-syntax``,
   ``--allow-pragmas``, ``--allow-short-circuit-operators``,
   ``--allow-mixed-located-var-declarations``,
   ``--allow-constant-initializer-expressions``,
   ``--allow-radix-case-labels``, ``--allow-paren-string-length``,
   ``--allow-struct-initializer-expressions``,
   ``--allow-fb-inheritance``, ``--allow-enum-explicit-values``,
   ``--allow-enum-base-type``, ``--allow-continue``,
   ``--allow-keywords-as-names``, ``--allow-radix-subrange-bounds``,
   ``--allow-conditional-return``, ``--allow-omitted-call-argument``,
   ``--allow-trailing-call-comma``, ``--allow-qualified-type-names``, and
   ``--allow-dotted-enum-value``.

   **Selects:** ``--policy-string-to-num-non-numeric ignore-trailing`` and
   ``--policy-string-to-num-failure zero`` — CODESYS stops parsing at the
   first invalid character and returns 0 for a string that is not valid in
   the target type.

**twincat**
   Beckhoff TwinCAT-compatible dialect. TwinCAT 3 is built on the CODESYS V3
   runtime, so it uses an Edition 2 base and enables the long-time-type
   keywords
   (:doc:`LTIME </reference/language/data-types/elementary/ltime>`,
   :doc:`LDT </reference/language/data-types/elementary/ldate-and-time>`, etc.)
   along with the extensions TwinCAT shares with CODESYS,
   such as curly-brace pragmas, C-style comments, and the ``AND_THEN`` /
   ``OR_ELSE`` short-circuit operators. Unlike ``codesys``, it does **not** enable the
   ``REF_TO`` / ``REF()`` / ``NULL`` reference extensions: TwinCAT spells
   references ``REFERENCE TO`` (bound with ``REF=``) and pointers
   ``POINTER TO`` (bound with ``ADR()``), which this dialect enables instead
   via ``--allow-reference-to``, ``--allow-pointer-to``, and
   ``--allow-adr``. As with ``codesys``,
   the implicit
   :doc:`__SYSTEM_UP_TIME </reference/extension-library/variables/system-uptime>`
   globals are not pre-bound, since they are an IronPLC runtime convention
   rather than a TwinCAT feature.

   **Enables:** ``--allow-c-style-comments``, ``--allow-missing-semicolon``,
   ``--allow-top-level-var-global``, ``--allow-constant-type-params``,
   ``--allow-empty-var-blocks``, ``--allow-time-as-function-name``,
   ``--allow-long-time-types``,
   ``--allow-reference-to``, ``--allow-pointer-to``, ``--allow-adr``,
   ``--allow-persistent-var``,
   ``--allow-int-to-bool-initializer``,
   ``--allow-sizeof``, ``--allow-cross-family-widening``,
   ``--allow-cross-family-conversion``, ``--allow-int-literal-to-bit-string``,
   ``--allow-bit-string-arithmetic``,
   ``--allow-partial-access-syntax``, ``--allow-pragmas``,
   ``--allow-short-circuit-operators``,
   ``--allow-mixed-located-var-declarations``,
   ``--allow-constant-initializer-expressions``,
   ``--allow-radix-case-labels``, ``--allow-paren-string-length``,
   ``--allow-struct-initializer-expressions``,
   ``--allow-fb-inheritance``, ``--allow-enum-explicit-values``,
   ``--allow-enum-base-type``, ``--allow-continue``,
   ``--allow-keywords-as-names``, ``--allow-radix-subrange-bounds``,
   ``--allow-conditional-return``, ``--allow-pou-header-semicolon``,
   ``--allow-omitted-call-argument``, ``--allow-trailing-call-comma``,
   ``--allow-qualified-type-names``, and ``--allow-dotted-enum-value``.

   **Selects:** ``--policy-string-to-num-non-numeric ignore-trailing`` and
   ``--policy-string-to-num-failure zero``, as ``codesys``.

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

.. include:: /includes/allow-flags.rst


Pass the flag when running :program:`ironplcc`:

.. code-block:: shell

   ironplcc check --allow-c-style-comments --allow-empty-var-blocks main.st

Or combine with a dialect:

.. code-block:: shell

   ironplcc check --dialect iec61131-3-ed3 --allow-c-style-comments main.st

See :doc:`/reference/compiler/ironplcc` for all compiler options.

---------------------------------
Selecting Behavior Policies
---------------------------------

An extension is syntax the parser accepts. A **behavior policy** is
different: it selects what a standard operation *does* where IEC 61131-3
leaves the result to the implementer and real platforms disagree. Each
policy has a small set of documented alternatives, the strict default is
the standard's result (or a runtime error where the standard says
"error"), and each dialect selects the alternative its platform documents.

Unlike a flag, which can only enable, a ``--policy-*`` selection replaces
the dialect's. Policies compose freely with each other and with any
dialect, and the selected alternative is compiled into the program, so a
:file:`.iplc` file behaves the same on every runtime that runs it.

``--policy-string-to-num-non-numeric``
   What ``STRING_TO_<numeric>`` treats as convertible when the string has
   characters that are not part of a numeric literal: ``reject`` (default),
   ``ignore-trailing``, or ``ignore-surrounding``.

``--policy-string-to-num-failure``
   What ``STRING_TO_<numeric>`` does when the string is not convertible:
   ``trap`` (default, runtime error
   :doc:`V4006 </reference/runtime/problems/V4006>`) or ``zero``.

The alternatives, their results, and the dialects' selections are on
:doc:`/reference/standard-library/functions/type-conversions`. To compile a
CODESYS program with the strict failure behavior:

.. code-block:: shell

   ironplcc compile --dialect codesys --policy-string-to-num-failure trap -o main.iplc main.st
