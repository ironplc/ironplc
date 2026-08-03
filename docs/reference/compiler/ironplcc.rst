========
ironplcc
========

Name
====

ironplcc --- IronPLC compiler

Synopsis
========

| :program:`ironplcc` [*OPTIONS*] *COMMAND*

Description
===========

:program:`ironplcc` is the IronPLC compiler command line interface. It checks
IEC 61131-3 source files for correctness and compiles them into bytecode
container (``.iplc``) files for execution by the :doc:`ironplcvm </reference/runtime/ironplcvm>` runtime.

Most developers will use :program:`ironplcc` through the Visual Studio Code
extension, but you can also use it directly, for example, to implement
a continuous integration pipeline.

When a command accepts multiple files, the files are treated as a single
compilation unit (essentially combined for analysis). Directory names can
be given to add all files in the given directory.

.. seealso::
   See :doc:`source-formats/index` for all supported source file formats.

Commands
========

Build Commands
--------------

:program:`ironplcc check` [*FILES*...]
   Check source files for syntax and semantic correctness without producing
   output. On success, the command produces no output.

:program:`ironplcc compile` [*FILES*...] ``-o`` *OUTPUT*
   Compile source files into a bytecode container (``.iplc``) file. Requires
   the ``--output`` (``-o``) flag to specify the output file path.

   .. warning::

      The compile command currently supports only trivial programs. Supported
      features include: ``PROGRAM`` declarations, ``INT`` variable declarations,
      assignment statements, integer literal constants, and the ``+`` (add)
      operator. Programs using other features will produce a code generation
      error.

Diagnostic Commands
-------------------

:program:`ironplcc echo` [*FILES*...]
   Parse source files and write the parsed representation to standard output.
   This is primarily useful for diagnostics and understanding the internal
   structure of the parsed files.

:program:`ironplcc tokenize` [*FILES*...]
   Tokenize source files and verify that all content matches a token.
   This is primarily useful for diagnostics and understanding the lexer
   behavior.

Informational Commands
----------------------

:program:`ironplcc dialects`
   Show available dialects and which features each enables. Use this to
   discover which ``--allow-*`` flags a dialect includes.

:program:`ironplcc version`
   Print the version number of the compiler.

Other Commands
--------------

:program:`ironplcc lsp` ``--stdio``
   Run in Language Server Protocol mode to integrate with development tools
   such as Visual Studio Code. Communication uses standard input/output.

Options
=======

``-v``, ``--verbose``
   Turn on verbose logging. Repeat the flag to increase verbosity (e.g.,
   ``-vvv``).

``-l`` *FILE*, ``--log-file`` *FILE*
   Write log output to the specified file instead of the terminal.

``--dialect`` *DIALECT*
   Select the language dialect. A dialect sets the IEC 61131-3 edition and a
   default set of vendor extensions. Individual ``--allow-*`` flags can
   override the dialect's defaults. Available values: ``iec61131-3-ed2``
   (default), ``iec61131-3-ed3``, ``rusty``, ``codesys``, ``twincat``. See
   :doc:`/explanation/enabling-dialects-and-features` for details.

``--allow-c-style-comments``
   Allow C-style comments (``//`` line comments and ``/* */`` block
   comments). This is a vendor extension not part of the IEC 61131-3
   standard.

``--allow-missing-semicolon``
   Allow missing semicolons after keyword statements like ``END_IF`` and
   ``END_STRUCT``. This is a vendor extension not part of the IEC 61131-3
   standard.

``--allow-top-level-var-global``
   Allow ``VAR_GLOBAL`` declarations at the top level of a file, outside of
   a ``CONFIGURATION`` block. This is a vendor extension not part of the
   IEC 61131-3 standard.

``--allow-constant-type-params``
   Allow constant references in type parameters (e.g., ``STRING[MY_CONST]``
   or ``ARRAY[1..MY_CONST] OF INT``). This is a vendor extension not part
   of the IEC 61131-3 standard.

``--allow-empty-var-blocks``
   Allow empty variable blocks (``VAR END_VAR``, ``VAR_INPUT END_VAR``,
   etc.). This is a vendor extension not part of the IEC 61131-3 standard.

``--allow-time-as-function-name``
   Allow ``TIME`` to be used as a function name (e.g., ``TIME()``).
   Required for OSCAT compatibility. This is a vendor extension not part
   of the IEC 61131-3 standard.

``--allow-ref-to``
   Allow ``REF_TO``, ``REF()``, and ``NULL`` syntax without enabling full
   Edition 3. This is a vendor extension useful when you need references
   but want to keep Edition 2 keyword handling for the rest of your code.

``--allow-reference-to``
   Allow the Beckhoff TwinCAT / CODESYS ``REFERENCE TO`` reference type and the
   ``REF=`` binding operator — the TwinCAT/CODESYS-facing alternative to
   ``--allow-ref-to``.

``--allow-ref-arithmetic``
   Allow arithmetic (``+``, ``-``) and ordering comparisons (``<``, ``>``,
   ``<=``, ``>=``) on ``REF_TO`` types. By default, only ``=`` and ``<>``
   are permitted on references.

``--allow-ref-stack-variables``
   Allow ``REF()`` on stack-allocated variables (``VAR_TEMP`` and function
   ``VAR_INPUT``/``VAR_OUTPUT``). This is a vendor extension not part of the
   IEC 61131-3 standard.

``--allow-ref-type-punning``
   Allow assigning between ``REF_TO`` types of different base types (type
   punning). This is a vendor extension not part of the IEC 61131-3 standard.

``--allow-int-to-bool-initializer``
   Allow integer literals ``0`` and ``1`` as ``BOOL`` variable initializers,
   treating ``0`` as ``FALSE`` and ``1`` as ``TRUE``. This is a vendor
   extension supported by CoDeSys, TwinCAT, RuSTy, and virtually every
   PLC runtime.

``--allow-sizeof``
   Allow the ``SIZEOF()`` operator that returns the size in bytes of a
   variable or type. This is a vendor extension supported by CODESYS,
   TwinCAT, and RuSTy.

``--allow-system-uptime-global``
   Expose ``__SYSTEM_UP_TIME`` (``TIME``) and ``__SYSTEM_UP_LTIME``
   (``LTIME``) as implicit ``VAR_GLOBAL`` values holding the VM's monotonic
   uptime. This is an IronPLC runtime convention.

``--allow-cross-family-widening``
   Allow implicit widening between bit-string and integer type families
   (e.g. ``BYTE`` to ``INT``, literal ``0`` to ``BYTE``). This is a vendor
   extension supported by CODESYS, TwinCAT, and RuSTy.

``--allow-partial-access-syntax``
   Allow IEC 61131-3:2013 partial-access bit syntax (``.%Xn``) as an alias
   for the short form ``.n``. Byte/word/dword/lword partial access (``.%Bn``,
   ``.%Wn``, ``.%Dn``, ``.%Ln``) is not yet supported.

``--allow-pragmas``
   Allow curly-brace pragmas such as ``{attribute 'qualified_only'}``. This
   is CODESYS-core syntax, inherited by TwinCAT and other CODESYS-based
   IDEs. A pragma is parsed and discarded like a comment; its contents are
   not interpreted.

``--allow-short-circuit-operators``
   Allow the ``AND_THEN`` short-circuit boolean operator, a Beckhoff/CODESYS
   extension that only evaluates its right operand when the left operand is
   ``TRUE``. ``ironplcc check`` fully supports it; codegen
   (``ironplcc compile``) does not yet implement short-circuit evaluation and
   refuses to compile it.

``--allow-mixed-located-var-declarations``
   Allow an ``AT``-located variable (e.g. ``AT %I*``) inside an otherwise
   plain ``VAR``/``VAR_INPUT``/``VAR_OUTPUT`` block, instead of requiring
   its own dedicated block. Produces
   :doc:`P4036 </reference/compiler/problems/P4036>` when mixed without
   this flag.

``--allow-constant-initializer-expressions``
   Allow a ``VAR`` initializer to be a constant expression (e.g.
   ``scaled : LREAL := SCALE*4.0;``) rather than only a bare literal.
   Folded to a literal at compile time; produces
   :doc:`P4037 </reference/compiler/problems/P4037>` when used without this
   flag, or :doc:`P4038 </reference/compiler/problems/P4038>` if the
   expression does not fully reduce to a constant.

``--allow-bit-string-case-labels``
   Allow a hex, binary, or octal bit-string literal (e.g. ``16#D012``,
   ``2#1010``) as a ``CASE`` label. The IEC 61131-3 standard permits only a
   subrange, decimal integer, or enumerated value here. Produces
   :doc:`P4041 </reference/compiler/problems/P4041>` when used without this
   flag.

``--allow-paren-string-length``
   Allow a string type's maximum length to be delimited with parentheses
   (``STRING(255)``, ``WSTRING(100)``) in addition to the standard square
   brackets. The IEC 61131-3 standard declares a string length only with
   brackets; the parenthesis form is a vendor extension. Produces
   :doc:`P4042 </reference/compiler/problems/P4042>` when used without this
   flag.

``--allow-struct-initializer-expressions``
   Allow a general (non-constant) expression — such as a pointer
   dereference plus member access (``pDevice^.Delta``) — as the value in a
   structured or call-style initializer's ``name := value`` pairs (e.g.
   ``tonDelta : TON := (PT := pDevice^.Delta);``). The IEC 61131-3 standard
   permits only a constant, enumerated value, array initializer, or nested
   structure initializer here; this vendor extension accepts a value
   computed at instantiation time. Produces
   :doc:`P4043 </reference/compiler/problems/P4043>` when used without this
   flag.

``--allow-oop-extensions``
   Allow CODESYS/TwinCAT OOP extensions: ``EXTENDS``/``IMPLEMENTS`` on
   ``FUNCTION_BLOCK`` and ``INTERFACE`` declarations. Parsed and registered
   as known types; inheritance, interface dispatch, and method/property
   declarations are not yet semantically supported (produces
   :doc:`P9004 </reference/compiler/problems/P9004>`). Enabled by
   ``--dialect=rusty`` and ``--dialect=codesys``.

Examples
========

1. Check a source file for correctness:

   .. code-block:: shell

      ironplcc check main.st

2. Check all files in a directory:

   .. code-block:: shell

      ironplcc check src/

3. Compile a source file to a bytecode container:

   .. code-block:: shell

      ironplcc compile main.st -o main.iplc

4. Compile with verbose logging to a file:

   .. code-block:: shell

      ironplcc -vv --log-file build.log compile main.st -o main.iplc

5. Inspect the parsed representation of a file:

   .. code-block:: shell

      ironplcc echo main.st

6. Check a source file using IEC 61131-3:2013 (Edition 3) features:

   .. code-block:: shell

      ironplcc check --dialect iec61131-3-ed3 main.st

7. Show available dialects and their features:

   .. code-block:: shell

      ironplcc dialects

See Also
========

* :doc:`/reference/runtime/ironplcvm` --- IronPLC virtual machine runtime
* :doc:`overview` --- Getting started tutorial
* :doc:`source-formats/index` --- Supported source file formats
* :doc:`problems/index` --- Compiler problem code index
