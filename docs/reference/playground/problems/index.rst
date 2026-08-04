==================
Problem Code Index
==================

The playground host layer — the WebAssembly wrapper that connects the browser to
the embedded compiler and virtual machine — reports errors with ``H####`` codes.

``H1xxx`` codes are **user-facing**: the input handed to the playground could not
be processed, and the message tells you what to correct. ``H9xxx`` codes are
**internal**: they signal a contract violation between the playground front end
and the WebAssembly module, or an internal failure, and should never appear
during normal use. If you encounter one, it is a bug in the playground.

.. problem-index:: H
