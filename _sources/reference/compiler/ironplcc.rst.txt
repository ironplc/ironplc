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

   .. note::

      The compile command supports Structured Text programs, including
      functions, function blocks, the standard library, and activated
      compatibility libraries. A program that uses a feature code
      generation does not yet support produces a code generation error
      rather than incorrect bytecode.

Diagnostic Commands
-------------------

:program:`ironplcc echo` [``--types``] [*FILES*...]
   Parse source files and write the parsed representation to standard output.
   This is primarily useful for diagnostics and understanding the internal
   structure of the parsed files.

   With ``--types``, the command analyzes the files and writes the analyzed
   program instead. A comment after each expression gives the type the
   compiler determined for it:

   .. code-block:: text

      flag := ( big (* LINT *) > total (* DINT -> LINT *) ) (* BOOL *) ;

   - ``(* DINT -> LINT *)`` marks an implicit conversion from the first type
     to the second.
   - ``CONSTANT`` marks a value the compiler knows, such as a literal:
     ``1 (* CONSTANT INT *)``.
   - ``?`` marks an expression whose type the compiler did not determine:
     ``(* ? *)``, or ``(* ? ANY_INT *)`` for a literal still of a generic
     type.

   The output is still Structured Text, but it shows the program as the
   compiler analyzed it rather than as written. Declarations appear in
   dependency order, named arguments become positional, and the compiler
   folds constant expressions. The output contains only the declarations from
   your files, not those of an activated compatibility library.

:program:`ironplcc tokenize` [*FILES*...]
   Tokenize source files and verify that all content matches a token.
   This is primarily useful for diagnostics and understanding the lexer
   behavior.

Informational Commands
----------------------

:program:`ironplcc dialects`
   Show available dialects and which features each enables. Use this to
   discover which ``--allow-*`` flags a dialect includes and which
   behavior policy alternatives it selects.

:program:`ironplcc version`
   Print the version number of the compiler.

Other Commands
--------------

:program:`ironplcc lsp` ``--stdio``
   Run in Language Server Protocol mode to integrate with development tools
   such as Visual Studio Code. Communication uses standard input/output.

Options
=======

.. include:: /includes/logging-options.rst

``--dialect`` *DIALECT*
   Select the language dialect. A dialect sets the IEC 61131-3 edition and a
   default set of extensions. Individual ``--allow-*`` flags can
   override the dialect's defaults. Available values: ``iec61131-3-ed2``
   (default), ``iec61131-3-ed3``, ``rusty``, ``codesys``, ``twincat``. See
   :doc:`/explanation/enabling-dialects-and-features` for details.

``--library`` *NAME*
   Activate a :doc:`compatibility library </reference/compatibility-libraries/index>`
   by name (for example ``--library Tc2_System``). Repeat the option to
   activate several libraries. Applies to the ``check`` and ``compile``
   commands, and to ``echo`` with ``--types``. Libraries referenced by a discovered project file are
   activated automatically and do not need this option.

.. include:: /includes/allow-flags.rst

``--policy-string-to-num-non-numeric`` *ALTERNATIVE*
   Select what ``STRING_TO_<numeric>`` treats as convertible when the string
   has non-numeric characters: ``reject`` (the whole string must be a
   literal; the default), ``ignore-trailing`` (convert the leading literal
   and ignore the rest), or ``ignore-surrounding`` (skip to the first
   literal, convert it, ignore the rest). A behavior policy: the dialect
   selects an alternative and this flag replaces it. See
   :doc:`/reference/standard-library/functions/type-conversions`.

``--policy-string-to-num-failure`` *ALTERNATIVE*
   Select what ``STRING_TO_<numeric>`` does when the string is not
   convertible: ``trap`` (halt with
   :doc:`V4006 </reference/runtime/problems/V4006>`; the default) or ``zero``
   (produce zero and continue). A behavior policy: the dialect selects an
   alternative and this flag replaces it. See
   :doc:`/reference/standard-library/functions/type-conversions`.

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

6. Inspect the type the compiler determined for each expression:

   .. code-block:: shell

      ironplcc echo --types main.st

7. Check a source file using IEC 61131-3:2013 (Edition 3) features:

   .. code-block:: shell

      ironplcc check --dialect iec61131-3-ed3 main.st

8. Show available dialects and their features:

   .. code-block:: shell

      ironplcc dialects

See Also
========

* :doc:`/reference/runtime/ironplcvm` --- IronPLC virtual machine runtime
* :doc:`overview` --- Getting started tutorial
* :doc:`source-formats/index` --- Supported source file formats
* :doc:`problems/index` --- Compiler problem code index
