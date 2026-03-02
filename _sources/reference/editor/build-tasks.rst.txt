===========
Build Tasks
===========

The IronPLC extension integrates with the Visual Studio Code build system to
compile IEC 61131-3 projects from within the editor. This lets you compile your
project to a bytecode container (``.iplc``) file without leaving Visual Studio Code.

.. warning::

   The compile command currently supports only trivial programs. Supported
   features include: ``PROGRAM`` declarations, ``INT`` variable declarations,
   assignment statements, integer literal constants, and the ``+`` (add)
   operator. Programs using other features will produce a code generation
   error.

Running a Build
===============

To compile the current project:

#. Open the Command Palette (:kbd:`Ctrl+Shift+P` or :kbd:`Cmd+Shift+P`).
#. Type "Run Build Task" and press :kbd:`Enter` (or use the shortcut
   :kbd:`Ctrl+Shift+B` / :kbd:`Cmd+Shift+B`).
#. Select :guilabel:`ironplc: compile` from the list of available tasks.

The extension runs :program:`ironplcc compile` on the workspace folder and
produces a ``.iplc`` file in the workspace root. The output file is named
after the workspace folder (for example, a folder named ``myproject`` produces
:file:`myproject.iplc`).

Build output appears in the :guilabel:`Terminal` panel.

Setting as the Default Build Task
=================================

If you use the build task frequently, you can set it as the default so that
:kbd:`Ctrl+Shift+B` runs it directly without prompting.

#. Open the Command Palette (:kbd:`Ctrl+Shift+P` or :kbd:`Cmd+Shift+P`).
#. Type "Configure Default Build Task" and press :kbd:`Enter`.
#. Select :guilabel:`ironplc: compile`.

This creates a :file:`.vscode/tasks.json` file in your workspace:

.. code-block:: json
   :caption: .vscode/tasks.json

   {
     "version": "2.0.0",
     "tasks": [
       {
         "type": "ironplc",
         "task": "compile",
         "group": {
           "kind": "build",
           "isDefault": true
         },
         "label": "ironplc: compile"
       }
     ]
   }

After configuring the default, pressing :kbd:`Ctrl+Shift+B` compiles
immediately.

.. seealso::

   For command-line usage of the compiler, see
   :doc:`/reference/compiler/overview`.
