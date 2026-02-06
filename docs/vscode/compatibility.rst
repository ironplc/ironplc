=================
IDE Compatibility
=================

The IronPLC extension is built for Visual Studio Code but should work with
editors that support VS Code extensions and the Language Server Protocol.

Visual Studio Code
==================

The primary supported editor. The extension requires VS Code version 1.75.0 or later.

Install from:

* `Visual Studio Marketplace <https://marketplace.visualstudio.com/items?itemName=ironplc.ironplc>`_
* VS Code Extensions view (search for "IronPLC")
* Command line: ``code --install-extension ironplc.ironplc``

Compatible Editors
==================

The following editors can run VS Code extensions and should work with IronPLC.
These have not been extensively tested, so please report any issues you encounter.

VS Codium
---------

The open-source build of VS Code without Microsoft telemetry. Install the extension
from the Open VSX Registry or by downloading the VSIX file from GitHub releases.

`VS Codium <https://vscodium.com/>`_

Cursor
------

An AI-enhanced fork of VS Code. Extensions from the VS Code marketplace should
install directly.

`Cursor <https://cursor.sh/>`_

Windsurf
--------

Another VS Code-based editor. Should support VS Code extensions.

`Windsurf <https://codeium.com/windsurf>`_

Theia
-----

Eclipse Theia is a framework for building cloud and desktop IDEs. Theia-based
editors support VS Code extensions through the Open VSX Registry.

`Eclipse Theia <https://theia-ide.org/>`_

Code Server
-----------

Run VS Code in a browser via code-server. The IronPLC extension should work,
though the compiler must be installed on the server machine.

`code-server <https://github.com/coder/code-server>`_

Gitpod / GitHub Codespaces
--------------------------

Cloud development environments based on VS Code. The extension can be installed,
but you will need to ensure the IronPLC compiler is available in the environment.

Language Server Protocol
========================

The IronPLC extension uses the Language Server Protocol (LSP) to communicate
with the compiler. This means the language analysis features could theoretically
be used with any LSP-compatible editor, not just VS Code.

The compiler provides an LSP server via the ``ironplcc lsp`` command. Advanced users
could configure other editors (Neovim, Emacs, Sublime Text, etc.) to use this
server directly.

Running the LSP server manually::

   ironplcc lsp

Or with verbose logging::

   ironplcc -v -v lsp

The server communicates via standard input/output using the LSP protocol.

Reporting Compatibility Issues
==============================

If you encounter issues using IronPLC with a compatible editor:

1. Note the editor name and version
2. Check if the IronPLC compiler is correctly installed (run ``ironplcc --version``)
3. Try enabling debug logging (see :doc:`settings`)
4. Report the issue on `GitHub <https://github.com/ironplc/ironplc/issues>`_
   with log output and reproduction steps
