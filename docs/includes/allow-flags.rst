``--allow-c-style-comments``
   Allow C-style comments (``//`` line comments and ``/* */`` block comments).
   These are not part of the IEC 61131-3 standard but are supported by many
   PLC environments.

``--allow-missing-semicolon``
   Allow missing semicolons after keyword statements like ``END_IF`` and
   ``END_STRUCT``. This is an extension, not part of the IEC 61131-3
   standard. It also allows a ``CASE`` branch with no statements at
   all (a label that falls straight through to the next label, ``ELSE``,
   or ``END_CASE``) -- strict IEC 61131-3 only allows this via an
   explicit empty statement (``5: ;``); this fills in the dropped ``;``.

``--allow-top-level-var-global``
   Allow :code:`VAR_GLOBAL` declarations at the top level of a file,
   outside of a :code:`CONFIGURATION` block. See
   :doc:`/reference/language/variables/scope`. This is an extension, not part of the IEC 61131-3 standard.

``--allow-constant-type-params``
   Allow constant references in type parameters such as array bounds and
   string lengths (e.g., ``ARRAY[1..MY_CONST] OF INT`` or
   ``STRING[MY_CONST]``). See :doc:`/reference/language/data-types/derived/array-types`.
   This is an extension, not part of the IEC 61131-3 standard.

``--allow-empty-var-blocks``
   Allow empty variable blocks (``VAR END_VAR``, ``VAR_INPUT END_VAR``, etc.).
   Some PLC environments permit variable blocks with no declarations.
   This is an extension, not part of the IEC 61131-3 standard.

``--allow-time-as-function-name``
   Allow ``TIME`` to be used as a function name (e.g., ``TIME()``).
   Required for OSCAT compatibility where ``TIME()`` reads the PLC system
   clock. This is an extension, not part of the IEC 61131-3 standard.

``--allow-long-time-types``
   Allow the IEC 61131-3:2013 long-time-type keywords
   :doc:`LTIME </reference/language/data-types/elementary/ltime>`,
   :doc:`LDATE </reference/language/data-types/elementary/ldate>`,
   :doc:`LTIME_OF_DAY </reference/language/data-types/elementary/ltime-of-day>`
   (``LTOD``), and
   :doc:`LDATE_AND_TIME </reference/language/data-types/elementary/ldate-and-time>`
   (``LDT``). Without this flag those words remain available as ordinary
   identifiers, so Edition 2 code may use them as names.

``--allow-ref-to``
   Allow ``REF_TO``, ``REF()``, and ``NULL`` syntax (standardized in
   IEC 61131-3:2013) without enabling the rest of Edition 3. This is useful
   when you need references but want to keep Edition 2 keyword handling for the
   rest of your code. See
   :doc:`/reference/language/data-types/derived/reference-types`.

``--allow-reference-to``
   Allow the Beckhoff TwinCAT / CODESYS ``REFERENCE TO`` reference type and the
   ``REF=`` binding operator. This is the TwinCAT/CODESYS-facing alternative to
   ``--allow-ref-to``: the two describe the same underlying reference but with
   different surface syntax. The compiler does not restrict flag combinations, so
   ``--allow-ref-to`` and ``--allow-reference-to`` may be set at once. See
   :doc:`/reference/language/data-types/derived/reference-types`.

``--allow-pointer-to``
   Allow the Beckhoff TwinCAT / CODESYS ``POINTER TO`` pointer type. A
   ``POINTER TO`` variable behaves like a ``REF_TO`` reference: reading or
   writing the target requires the explicit dereference operator (``^``),
   unlike ``REFERENCE TO``, which dereferences implicitly. Pointers are
   bound with the ``ADR()`` operator (``--allow-adr``) or the
   ``REF()``/``NULL`` forms from ``--allow-ref-to``. See
   :doc:`/reference/language/data-types/derived/reference-types`.

``--allow-adr``
   Allow the ``ADR()`` address-of operator, which returns a typed pointer to
   a variable for assignment to a ``POINTER TO`` variable
   (``--allow-pointer-to``). Unlike TwinCAT's untyped ``PVOID`` result,
   ``ADR(x)`` in IronPLC has the type ``POINTER TO`` *typeof(x)* and is
   type-checked against the destination pointer's target type. Addresses of
   sub-objects (array elements, structure fields) and pointer arithmetic are
   not supported and are rejected with a diagnostic. See
   :doc:`/reference/extension-library/functions/adr`.

``--allow-persistent-var``
   .. include:: /includes/persistent-var-flag.rst

``--allow-ref-arithmetic``
   Allow arithmetic (``+``, ``-``) and ordering comparisons (``<``, ``>``,
   ``<=``, ``>=``) on ``REF_TO`` types. By default, only ``=`` and ``<>``
   are permitted on references.

``--allow-ref-stack-variables``
   Allow ``REF()`` on stack-allocated variables (``VAR_TEMP`` and function
   ``VAR_INPUT``/``VAR_OUTPUT``). Required for OSCAT patterns where the
   reference does not escape the call. This is an extension, not part of the IEC 61131-3 standard.

``--allow-ref-type-punning``
   Allow assigning between ``REF_TO`` types of different base types (type
   punning), such as reinterpreting the bits of a ``REAL`` through a
   ``REF_TO DWORD``. This is an extension, not part of the IEC 61131-3 standard.

``--allow-int-to-bool-initializer``
   Allow integer literals ``0`` and ``1`` as ``BOOL`` variable initializers
   (e.g., ``debug : BOOL := 0;``). The compiler rewrites ``0`` to ``FALSE``
   and ``1`` to ``TRUE``. This is a universal extension supported by
   CoDeSys, TwinCAT, RuSTy, and virtually every PLC runtime.

``--allow-sizeof``
   Allow the ``SIZEOF()`` operator that returns the size in bytes of a
   variable or type. This is an extension supported by CODESYS,
   TwinCAT, and RuSTy. See
   :doc:`/reference/extension-library/functions/sizeof`.

``--allow-system-uptime-global``
   Expose ``__SYSTEM_UP_TIME`` (``TIME``) and ``__SYSTEM_UP_LTIME``
   (``LTIME``) as implicit ``VAR_GLOBAL`` values holding the VM's monotonic
   uptime. This is an IronPLC runtime convention.

``--allow-cross-family-widening``
   Allow implicit widening from a bit-string type to a strictly wider
   integer type. For example, passing a ``BYTE`` variable where an ``INT``
   parameter is expected. This is an extension supported by CODESYS,
   TwinCAT, and RuSTy.

``--allow-cross-family-conversion``
   Allow implicit conversion between ``UDINT`` and ``DWORD``, in both
   directions. The two types share a 32-bit slot, so this converts rather
   than widens, and it is the one case where an integer converts implicitly
   to a bit-string. See :doc:`/explanation/type-conversions`. This is an
   extension supported by CODESYS, TwinCAT, and RuSTy.

``--allow-int-literal-to-bit-string``
   Allow a bare integer literal where a bit-string type is expected. For
   example, passing ``0`` where a ``BYTE`` parameter is expected. This is
   an extension supported by CODESYS, TwinCAT, and RuSTy.

``--allow-bit-string-arithmetic``
   Allow the arithmetic operators ``+``, ``-``, ``*`` and ``/`` (and
   ``ADD``, ``SUB``, ``MUL`` and ``DIV``) on bit-string operands. For
   example, ``counter := counter + 1`` where ``counter`` is a ``BYTE``. A
   ``BYTE``, ``WORD``, ``DWORD`` or ``LWORD`` operand is treated as the
   unsigned integer of its width, and the result keeps the bit-string type:
   ``BYTE#255 + 1`` is ``BYTE#0``. ``BOOL`` and ``MOD`` are not included.
   See :doc:`/explanation/type-conversions`. This is an extension supported
   by CODESYS, TwinCAT, and RuSTy.

``--allow-partial-access-syntax``
   Allow IEC 61131-3:2013 partial-access syntax: the bit form ``.%Xn``
   (e.g., ``myByte.%X3`` to access bit 3 of a ``BYTE``, equivalent to the
   short form ``.n``) and the byte, word, double word, and long word forms
   ``.%Bn``, ``.%Wn``, ``.%Dn``, and ``.%Ln`` (e.g., ``myDword.%B2`` to
   access byte 2 of a ``DWORD``). See
   :doc:`/reference/language/structured-text/bit-access`.

``--allow-pragmas``
   Allow curly-brace pragmas such as ``{attribute 'qualified_only'}`` and
   ``{attribute 'strict'}``. These are CODESYS-core syntax (documented by
   CODESYS itself, and inherited by any IDE built on the CODESYS V3 runtime,
   including Beckhoff TwinCAT and Schneider Electric Machine Expert). A
   pragma is parsed and discarded like a comment — its contents are not yet
   interpreted. Pragmas do not nest; an unclosed ``{`` still produces a parse
   error.

``--allow-short-circuit-operators``
   Allow the ``AND_THEN`` and ``OR_ELSE`` short-circuit boolean
   operators, a Beckhoff/CODESYS extension. Unlike plain ``AND`` and
   ``OR`` (which always evaluate both operands), ``AND_THEN`` only
   evaluates its right operand when the left operand is ``TRUE``, and
   ``OR_ELSE`` only when the left operand is ``FALSE`` — commonly used
   to guard a dereference (``ptr <> 0 AND_THEN ptr^ = 99``). The
   spelling is preserved end to end: the operators are *not* normalized
   to ``AND``/``OR``, since the short-circuit behavior is a real,
   externally-visible difference in TwinCAT/CODESYS. Compiled code
   branches around the right operand, so the guarded expression above
   never dereferences a null pointer.

   Short-circuiting applies to ``BOOL`` operands. ``AND`` and ``OR`` are
   also the bit-string operators, and skipping an operand has no meaning
   for a bit-string result, so ``AND_THEN``/``OR_ELSE`` on bit-string
   operands evaluate both operands and produce the same value ``AND``
   and ``OR`` produce. Like ``AND`` and ``OR``, they accept only ``BOOL``
   and bit-string operands: an integer operand is :doc:`P4049
   </reference/compiler/problems/P4049>`.

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
   Allow a ``VAR`` initializer or a ``TYPE`` default to be a constant
   *expression* — the name of a declared ``CONSTANT`` variable
   (e.g. ``level : UDINT := DEFAULT_LEVEL;``)
   or arithmetic between literals and such names
   (e.g. ``scaled : LREAL := SCALE*4.0;``) — rather than only a bare
   literal. The IEC 61131-3 standard's initializer grammar permits only
   literals in this position; this extension folds the expression
   to a literal at compile time. Using this form without the flag produces
   :doc:`P4037 </reference/compiler/problems/P4037>`; if the expression
   does not fully reduce to a constant (e.g. it references a
   non-``CONSTANT`` variable), it produces
   :doc:`P4038 </reference/compiler/problems/P4038>`; if a named constant's
   type is not compatible with the variable's, it produces
   :doc:`P4022 </reference/compiler/problems/P4022>`.

``--allow-radix-case-labels``
   Allow a hex, binary, or octal bit-string literal (e.g. ``16#D012``,
   ``2#1010``, ``8#17``) as a ``CASE`` label. The IEC 61131-3 standard
   grammar for a case label permits only a subrange, a *decimal*
   ``signed_integer``, or an enumerated value; radix-prefixed literals are
   separate productions the standard does not include here. Real
   TwinCAT/CODESYS code uses them. The flag also allows them as the bounds
   of a subrange label (``16#01..16#0F:``). Without this flag, such a label
   produces :doc:`P4041 </reference/compiler/problems/P4041>`. A plain
   decimal label (``5:``) is standard syntax and is always allowed.

``--allow-paren-string-length``
   Allow a string type's maximum length to be delimited with parentheses
   (``STRING(255)``, ``WSTRING(100)``) in addition to the standard square
   brackets (``STRING[255]``). The IEC 61131-3 standard grammar declares a
   string length only with brackets; the parenthesis form is a dialect
   extension. Without this flag, the parenthesis form produces
   :doc:`P4042 </reference/compiler/problems/P4042>`. The bracket form is
   standard syntax and is always allowed, and the delimiters must match
   (``STRING[255)`` is always a syntax error). The renderer normalizes the
   parenthesis form to brackets. The flag applies in every position a string
   type can appear, including as an array element type
   (``ARRAY[1..10] OF STRING(255)``), a function return type, and a
   ``TYPE`` alias.

``--allow-radix-subrange-bounds``
   Allow a hex, binary, or octal literal (e.g. ``16#FF``, ``2#1010``,
   ``8#17``) as a bound of a subrange type (``INT (16#00..16#FF)``) or of an
   array dimension (``ARRAY[16#0..16#F] OF INT``). The IEC 61131-3 standard
   grammar defines a subrange's bounds as *decimal* ``signed_integer``\ s.
   Without this flag, such a bound produces
   :doc:`P4096 </reference/compiler/problems/P4096>`. The bounds of a
   ``CASE`` subrange label are allowed by ``--allow-radix-case-labels``
   instead.

``--allow-struct-initializer-expressions``
   Allow a general (non-constant) expression — such as a pointer
   dereference plus member access (``pDevice^.Delta``) — as the value in a
   structured or call-style initializer's ``name := value`` pairs (e.g.
   ``tonDelta : TON := (PT := pDevice^.Delta);``). The IEC 61131-3 standard
   grammar for a structured initializer value permits only a constant,
   enumerated value, array initializer, or nested structure initializer;
   a value computed at instantiation time is an extension used by
   TwinCAT/CODESYS. Without this flag, such a value produces
   :doc:`P4043 </reference/compiler/problems/P4043>`. A constant value is
   standard syntax and is always allowed. The ``pDevice^.Delta`` example
   spells its pointer with ``REF_TO``, which is a separate extension needing
   ``--allow-ref-to``; not every dialect that enables one enables the other,
   and some spell the same declaration ``POINTER TO FB_Device``.
   :program:`ironplcc dialects` lists the flags each dialect enables.

``--allow-fb-inheritance``
   Allow the IEC 61131-3:2013 :doc:`object-oriented syntax
   </reference/language/object-orientation/index>`:
   ``EXTENDS``/``IMPLEMENTS`` and qualifiers (``ABSTRACT``, ``FINAL``,
   ``PUBLIC``, ...) on ``FUNCTION_BLOCK`` declarations, ``INTERFACE``
   declarations, ``METHOD`` and ``PROPERTY`` declarations, and
   ``THIS``/``SUPER``. Support beyond parsing varies by keyword — see
   :doc:`/reference/language/object-orientation/index` for what each one
   analyzes and executes today; the parts that are parsed but not yet
   analyzed produce problem
   :doc:`P9999 </reference/compiler/problems/P9999>` rather than a parse
   error. A function block qualifier produces
   :doc:`P4062 </reference/compiler/problems/P4062>` when used without this
   flag. Enabled by ``--dialect=iec61131-3-ed3``, ``--dialect=rusty``,
   ``--dialect=codesys``, and ``--dialect=twincat``.

``--allow-enum-explicit-values``
   Allow a member of an :doc:`enumeration
   </reference/language/data-types/derived/enumerated-types>` declaration to
   be given an explicit value (e.g. ``(Deutsch := 1, English := 2)``). The
   IEC 61131-3:2003 (Edition 2) grammar lets an enumeration list only bare
   member names, which take consecutive values starting at zero; explicit
   values were added in Edition 3. Without this flag, an explicit value
   produces
   :doc:`P4055 </reference/compiler/problems/P4055>`. A list of bare member
   names is standard syntax and is always allowed, as is a default value
   (``(RED, GREEN, BLUE) := GREEN``), which selects the initial member rather
   than renumbering one.

``--allow-enum-base-type``
   Allow the base-type suffix on an :doc:`enumeration
   </reference/language/data-types/derived/enumerated-types>` declaration
   (e.g. ``(A, B) WORD``), naming a base type; IronPLC stores every
   enumeration as a ``DINT`` whatever it names. IEC 61131-3 has no such form — an enumerated specification is a list
   of member names or another enumerated type's name, and nothing may follow
   it — so the suffix is an extension. Unlike
   ``--allow-enum-explicit-values`` above it is not an Edition 3 addition;
   the two constructs often appear together but are separate questions.
   Without this flag it produces
   :doc:`P4056 </reference/compiler/problems/P4056>`, and the compiler sizes
   the type from the members instead.

``--allow-continue``
   Allow the :doc:`CONTINUE </reference/language/structured-text/continue>`
   statement, which goes on with the next iteration of the innermost ``FOR``,
   ``WHILE``, or ``REPEAT`` loop.
   ``CONTINUE`` was added in Edition 3. Without this flag, ``CONTINUE`` is an
   ordinary identifier, so a ``CONTINUE;`` statement is a syntax error and a
   variable may be named ``continue``.

``--allow-keywords-as-names``
   Allow the keywords ``ON``, ``STEP``, ``R_EDGE`` and ``F_EDGE`` as names of
   variables and enumeration members, as in ``TYPE M : (Off, On); END_TYPE``
   or ``VAR_INPUT step : DINT; END_VAR``. IEC 61131-3 reserves its keywords
   for their own use, while other toolchains do not reserve these four.
   Each keyword keeps its keyword role in the same program
   (``RESOURCE r ON PLC``, an ``R_EDGE`` input). Without this flag such a
   name produces :doc:`P4095 </reference/compiler/problems/P4095>`.

``--allow-conditional-return``
   Allow :doc:`RETURN(condition) </reference/language/structured-text/return>`,
   a ``RETURN`` that happens only when the ``BOOL`` condition is ``TRUE``, as
   if written ``IF condition THEN RETURN; END_IF;``. IEC 61131-3 defines
   ``RETURN`` with no operand. The condition is never the return value of a
   function. Without this flag it produces
   :doc:`P4098 </reference/compiler/problems/P4098>`.

``--allow-pou-header-semicolon``
   Allow a semicolon after the header of a ``PROGRAM``, ``FUNCTION``,
   ``FUNCTION_BLOCK`` or ``METHOD``, before its first variable block, as in
   ``FUNCTION_BLOCK FB_Motor EXTENDS FB_Base;``. The IEC 61131-3 standard
   grammar puts no terminator after a header. The semicolon has no meaning:
   the declaration is the same as without it. Without this flag it produces
   :doc:`P4100 </reference/compiler/problems/P4100>`. A semicolon that a
   variable block does not follow is the standard empty statement at the start
   of the body and is always allowed.

``--allow-omitted-call-argument``
   Allow a call argument with no value (``b :=,`` for an input, ``Error=> ,``
   for an output). It is the same as leaving the argument out. The IEC 61131-3
   standard grammar gives every argument a value. Without this flag it produces
   :doc:`P4102 </reference/compiler/problems/P4102>`.

``--allow-trailing-call-comma``
   Allow a comma before the closing parenthesis of a call's argument list. The
   IEC 61131-3 standard grammar has no comma after the last argument. Without
   this flag it produces :doc:`P4103 </reference/compiler/problems/P4103>`.

``--allow-dotted-enum-value``
   Allow ``Type.value`` as the spelling of a qualified enumerated value,
   besides the standard ``Type#value``. Without this flag it produces
   :doc:`P4104 </reference/compiler/problems/P4104>`.

``--allow-extensible-concat``
   Allow :doc:`CONCAT </reference/standard-library/functions/concat>` of more
   than two strings, as in ``CONCAT(a, b, c)``. IEC 61131-3 declares
   ``CONCAT`` extensible, so the ``iec61131-3-ed2``, ``iec61131-3-ed3`` and
   ``rusty`` dialects turn this flag on. CODESYS and TwinCAT declare
   ``CONCAT`` with two inputs, so the ``codesys`` and ``twincat`` dialects
   leave it off. Without this flag a third input produces
   :doc:`P4018 </reference/compiler/problems/P4018>`.
