==========
ironplcvm
==========

Name
====

ironplcvm --- IronPLC bytecode virtual machine

Synopsis
========

| :program:`ironplcvm` [*OPTIONS*] *COMMAND*

Description
===========

:program:`ironplcvm` is the IronPLC virtual machine runtime. It loads and
executes compiled bytecode container (``.iplc``) files produced by
:doc:`ironplcc </reference/compiler/ironplcc>`.

The runtime follows the IEC 61131-3 execution model. Each scheduling round,
the runtime checks which tasks are due based on elapsed time and executes
them in priority order.

By default, :program:`ironplcvm` runs continuously until interrupted with
:kbd:`Ctrl+C`. Use ``--scans`` to limit execution to a fixed number of
scheduling rounds.

Commands
========

:program:`ironplcvm run` [*OPTIONS*] *FILE*
   Load and execute a bytecode container (``.iplc``) file.

   ``--dump-vars`` *FILE*
      Write all variable values to the specified file after execution stops.
      The output contains one variable per line in the format ``var[N]: VALUE``.
      Variables are dumped on both normal shutdown and after a VM trap.

   ``--scans`` *N*
      Run exactly *N* scheduling rounds then stop. Without this option, the
      runtime runs continuously until interrupted with :kbd:`Ctrl+C`.

:program:`ironplcvm version`
   Print the version number of the virtual machine.

Options
=======

``-v``, ``--verbose``
   Turn on verbose logging. Repeat the flag to increase verbosity (e.g.,
   ``-vvv``).

``-l`` *FILE*, ``--log-file`` *FILE*
   Write log output to the specified file instead of the terminal.

Examples
========

1. Run a compiled program:

   .. code-block:: shell

      ironplcvm run main.iplc

2. Run for a single scan and dump variable values:

   .. code-block:: shell

      ironplcvm run main.iplc --scans 1 --dump-vars output.txt

3. Run with verbose logging:

   .. code-block:: shell

      ironplcvm -vv run main.iplc

See Also
========

* :doc:`/reference/compiler/ironplcc` --- IronPLC compiler
* :doc:`overview` --- Runtime overview
