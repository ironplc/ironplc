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

Other Commands
--------------

:program:`ironplcc lsp` ``--stdio``
   Run in Language Server Protocol mode to integrate with development tools
   such as Visual Studio Code. Communication uses standard input/output.

:program:`ironplcc version`
   Print the version number of the compiler.

Options
=======

``-v``, ``--verbose``
   Turn on verbose logging. Repeat the flag to increase verbosity (e.g.,
   ``-vvv``).

``-l`` *FILE*, ``--log-file`` *FILE*
   Write log output to the specified file instead of the terminal.

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

See Also
========

* :doc:`/reference/runtime/ironplcvm` --- IronPLC virtual machine runtime
* :doc:`basicusage` --- Getting started tutorial
* :doc:`source-formats/index` --- Supported source file formats
* :doc:`problems/index` --- Compiler problem code index
