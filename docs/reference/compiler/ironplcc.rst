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
   (default), ``iec61131-3-ed3``, ``rusty``. See
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

``--allow-pointer-arithmetic``
   Allow arithmetic (``+``, ``-``) and ordering comparisons (``<``, ``>``,
   ``<=``, ``>=``) on ``REF_TO`` types. By default, only ``=`` and ``<>``
   are permitted on references.

``--allow-int-to-bool-initializer``
   Allow integer literals ``0`` and ``1`` as ``BOOL`` variable initializers,
   treating ``0`` as ``FALSE`` and ``1`` as ``TRUE``. This is a vendor
   extension supported by CoDeSys, TwinCAT, RuSTy, and virtually every
   PLC runtime.

``--allow-sizeof``
   Allow the ``SIZEOF()`` operator that returns the size in bytes of a
   variable or type. This is a vendor extension supported by CODESYS,
   TwinCAT, and RuSTy.

``--allow-cross-family-widening``
   Allow implicit widening between bit-string and integer type families
   (e.g. ``BYTE`` to ``INT``, literal ``0`` to ``BYTE``). This is a vendor
   extension supported by CODESYS, TwinCAT, and RuSTy.

``--allow-partial-access-syntax``
   Allow IEC 61131-3:2013 partial-access bit syntax (``.%Xn``) as an alias
   for the short form ``.n``. Enabled by ``--dialect=iec61131-3-ed3`` and
   ``--dialect=rusty``.

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
