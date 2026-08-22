===========
ironplcdap
===========

.. include:: /includes/debugging-in-development.rst

Name
====

ironplcdap --- IronPLC debug server

Synopsis
========

| :program:`ironplcdap`

Description
===========

:program:`ironplcdap` is the IronPLC debug server. It speaks the
`Debug Adapter Protocol <https://microsoft.github.io/debug-adapter-protocol/>`_
(DAP) on standard input and output, so any DAP-capable editor can debug an
IEC 61131-3 program with it.

The server takes no command-line arguments. The program to debug arrives in
the DAP ``launch`` request as a path to a compiled bytecode container
(``.iplc``) file.

:program:`ironplcdap` installs alongside :program:`ironplcc` and
:program:`ironplcvm`. Most developers never run it directly --- the
:doc:`extension </reference/editor/debugging>` starts it for you.

The server embeds the same virtual machine as
:doc:`ironplcvm <ironplcvm>`, so a program computes the same results under the
debugger as it does in production, with one difference: the debugger stops
it.

Launch Arguments
================

The ``launch`` request accepts these arguments:

.. list-table::
   :header-rows: 1
   :widths: 20 15 65

   * - Argument
     - Type
     - Description
   * - ``program``
     - string
     - **Required.** Path to a compiled ``.iplc`` container. The server does
       not compile source files.
   * - ``stopOnEntry``
     - boolean
     - Pause before the first scan cycle begins. Defaults to ``false``.
   * - ``scanLimit``
     - number
     - Stop after this many scan cycles. ``0`` means unlimited.

Launch Preconditions
====================

The server checks two conditions before it starts a program. Each failure
answers the ``launch`` request with an error carrying an IronPLC problem code:

#. **The container must carry debug information.** Without it there are no
   source lines or variable names to debug against. See :doc:`problems/V6009`.
#. **The container must declare exactly one program instance.** See
   :doc:`problems/V6010`.

A ``launch`` request with no usable ``program`` path reports
:doc:`problems/V6008`.

Supported Requests
==================

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Request
     - Notes
   * - ``initialize``
     - Reports ``supportsConfigurationDoneRequest``.
   * - ``launch``
     - Loads the container and starts the virtual machine.
   * - ``configurationDone``
     - Begins execution.
   * - ``setBreakpoints``
     - After ``launch`` and before ``configurationDone``, and at any pause ---
       not before ``launch``, which answers ``requestNotApplicable``.
       Line-level. The response echoes the line the breakpoint bound to, which
       may differ from the line requested, and reports ``verified: false`` for
       a line that could not be bound.
   * - ``threads``
     - Reports a single thread named ``plc``.
   * - ``stackTrace``
     - Frames named by POU, with the source file and line.
   * - ``scopes``
     - Two scopes: ``Program`` and ``Runtime``.
   * - ``variables``
     - Named, typed values for the requested scope.
   * - ``continue``, ``next``, ``stepIn``, ``stepOut``
     - Accepted while paused.
   * - ``disconnect``
     - Accepted at any time; ends the session.

Requests are answered only when the program is stopped --- at a breakpoint, a
step landing, entry, a trap, or completion. The server runs a single thread
and reads the next request at the next stop.

A request the server does not support, and a supported request sent when the
program is not in a state to accept it, both answer with the DAP error
``requestNotApplicable``. ``pause``, ``setVariable``, ``evaluate``, and
``restart`` are recognized but always refused.

Inspection requests (``threads``, ``stackTrace``, ``scopes``, ``variables``)
are also accepted after a trap, so a failure can be examined. Execution
control is not: a trapped program cannot resume.

Using Another Editor
====================

Any DAP client can drive :program:`ironplcdap`. Configure the client to launch
the executable and speak DAP over its standard input and output, then send a
``launch`` request naming a container:

.. code-block:: json

   {
     "seq": 2,
     "type": "request",
     "command": "launch",
     "arguments": {
       "program": "/path/to/myproject.iplc",
       "stopOnEntry": true
     }
   }

Compiling source to a container is the client's job. The
:doc:`extension </reference/editor/debugging>` does this before it launches;
another editor must run :program:`ironplcc compile` itself, or debug a
container built ahead of time.

Set breakpoints with ``setBreakpoints`` using the path of the source file, not
the container. The container records the file each line came from, and the
server matches the requested path against those records.

Build a container to debug with :program:`ironplcc`:

.. code-block:: shell

   ironplcc compile . -o myproject.iplc

See Also
========

* :doc:`/reference/editor/debugging` --- debugging from the extension
* :doc:`ironplcvm` --- run a program without the debugger
* :doc:`/reference/compiler/ironplcc` --- IronPLC compiler
* :doc:`problems/index` --- runtime problem codes
