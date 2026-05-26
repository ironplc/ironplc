.. _installation steps target:

============
Installation
============

.. note::

   The IronPLC VS Code extension is currently unavailable on the Visual
   Studio Marketplace. Install via:

   - `Open VSX <https://open-vsx.org/extension/ironplc/ironplc>`_ (Cursor,
     Windsurf, VSCodium)
   - Direct VSIX from `IronPLC GitHub releases`_

   Existing installs continue to work; automatic updates from the
   Marketplace are paused. We are working with Microsoft to restore the
   listing.

IronPLC supports the following platforms:

- Windows (x64, arm64)
- macOS (x64, arm64)
- Linux (x64)

-------------
Prerequisites
-------------

The first step is to install a supported development environment:

- `Visual Studio Code <https://code.visualstudio.com/>`_
- `Cursor <https://www.cursor.com/>`_

Other development environments that support VS Code extensions (via the
`Open VSX Registry <https://open-vsx.org/>`_) also work. The instructions
below use Visual Studio Code, but the steps are the same in all supported
environments.

-------------
Install Steps
-------------

Follow the steps below to install IronPLC.

.. tab:: Windows

   .. rubric:: Install IronPLC CLI

   #. Download the latest IronPLC installer from `IronPLC GitHub releases`_:
   
      * x64 :download_artifact:`ironplcc-x86_64-windows.exe`

      * Arm64 :download_artifact:`ironplcc-aarch64-windows.exe`

   #. Run the installer and follow the prompts to complete
      installation of the CLI.

   .. rubric:: Install IronPLC Extension

   #. Download the latest IronPLC extension
      :download_artifact:`ironplc-vscode-extension.vsix` from
      `IronPLC GitHub releases`_.

   Run your development environment, then:

   #. Go to the Extensions view by clicking on the Extensions icon in
      :guilabel:`Activity Bar` on the side of the window or using the
      View: Extensions command (:kbd:`Ctrl+Shift+X`).
   #. In the Extensions view, select :menuselection:`... (View and More Actions) --> Install from VSIX...` button.
   #. In the :guilabel:`Install from VSIX` dialog, select the VSIX file you downloaded earlier.

.. tab:: macOS

   .. rubric:: Install IronPLC CLI

   Option 1 — Homebrew (recommended):

   #. Go to `Homebrew <https://brew.sh/>`_ then follow the instructions to
      install Homebrew.
   #. Open a Terminal and run:

      .. code-block:: sh

         brew tap ironplc/tap
         brew install ironplc

   Option 2 — install script:

   Open a Terminal and run:

   .. code-block:: sh

      curl -fsSL https://www.ironplc.com/install.sh | sh

   This installs ``ironplcc``, ``ironplcvm``, and ``ironplcmcp`` into
   ``$HOME/.ironplc/bin`` and adds that directory to your ``PATH`` via
   your shell profile.

   .. rubric:: Install IronPLC Extension

   #. Download the latest IronPLC extension
      :download_artifact:`ironplc-vscode-extension.vsix` from
      `IronPLC GitHub releases`_.

   Run your development environment, then:

   #. Go to the Extensions view by clicking on the Extensions icon in
      :guilabel:`Activity Bar` on the side of the window or using the
      View: Extensions command (:kbd:`⌘+Shift+X`).
   #. In the Extensions view, select :menuselection:`... (View and More Actions) --> Install from VSIX...` button.
   #. In the :guilabel:`Install from VSIX` dialog, select the VSIX file you downloaded earlier.

.. tab:: Linux

   .. rubric:: Install IronPLC CLI

   Open a terminal and run:

   .. code-block:: sh

      curl -fsSL https://www.ironplc.com/install.sh | sh

   This installs ``ironplcc``, ``ironplcvm``, and ``ironplcmcp`` into
   ``$HOME/.ironplc/bin`` and adds that directory to your ``PATH`` via
   your shell profile.

   To install a specific version:

   .. code-block:: sh

      curl -fsSL https://www.ironplc.com/install.sh | IRONPLC_VERSION=v0.201.0 sh

   Prebuilt binaries are currently provided for x86_64 Linux only.

   .. rubric:: Install IronPLC Extension

   #. Download the latest IronPLC extension
      :download_artifact:`ironplc-vscode-extension.vsix` from
      `IronPLC GitHub releases`_.

   Run your development environment, then:

   #. Go to the Extensions view by clicking on the Extensions icon in
      :guilabel:`Activity Bar` on the side of the window or using the
      View: Extensions command (:kbd:`Ctrl+Shift+X`).
   #. In the Extensions view, select :menuselection:`... (View and More Actions) --> Install from VSIX...` button.
   #. In the :guilabel:`Install from VSIX` dialog, select the VSIX file you downloaded earlier.

--------------------------------------
Next Steps
--------------------------------------

You are ready to start programming. In the next chapter, you will learn how
PLC programs work before writing your first one.

Continue to :doc:`sense-control-actuate`.

.. _IronPLC GitHub releases: https://github.com/ironplc/ironplc/releases/
