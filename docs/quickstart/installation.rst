.. _installation steps target:

============
Installation
============

.. note::

   The IronPLC extension is published to two registries under two different
   names:

   - `Visual Studio Marketplace
     <https://marketplace.visualstudio.com/items?itemName=ironplc.ironplc-vscode>`_
     — listed as :guilabel:`IronPLC IDE` (extension ID
     ``ironplc.ironplc-vscode``)
   - `Open VSX <https://open-vsx.org/extension/ironplc/ironplc>`_ — listed as
     :guilabel:`IronPLC` (extension ID ``ironplc.ironplc``)

   Both listings are the same extension; only the name differs, because the
   two registries are separate namespaces. Visual Studio Code installs from
   the Marketplace, and editors such as Cursor, Kiro, Devin, and VSCodium
   install from Open VSX. A VSIX for manual installation is attached to every
   `IronPLC GitHub releases`_.

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
- `Kiro <https://kiro.dev/>`_
- `Devin <https://devin.ai/>`_ (formerly Windsurf)

Other development environments that support VS Code extensions (via the
`Open VSX Registry <https://open-vsx.org/>`_) also work. The instructions
below use Visual Studio Code, but the steps are the same in all supported
environments.

.. note::

   In Cursor, IronPLC works in the Cursor IDE interface. It is not available
   in Cursor's newer agent-first interface. Switch to the IDE interface to
   use IronPLC.

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

   Run your development environment, then:

   #. Go to the Extensions view by clicking on the Extensions icon in
      :guilabel:`Activity Bar` on the side of the window or using the
      View: Extensions command (:kbd:`Ctrl+Shift+X`).
   #. Search for ``ironplc``.
   #. Select :guilabel:`IronPLC IDE` in Visual Studio Code, or
      :guilabel:`IronPLC` in editors that install from Open VSX, then select
      :guilabel:`Install`.

   .. rubric:: Install IronPLC Extension from a VSIX

   If your development environment cannot reach either registry, install the
   extension manually:

   #. Download the latest IronPLC extension
      :download_artifact:`ironplc-vscode-extension.vsix` from
      `IronPLC GitHub releases`_.
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

   This installs IronPLC into ``$HOME/.ironplc/bin`` and adds that
   directory to your ``PATH`` via your shell profile.

   .. rubric:: Install IronPLC Extension

   Run your development environment, then:

   #. Go to the Extensions view by clicking on the Extensions icon in
      :guilabel:`Activity Bar` on the side of the window or using the
      View: Extensions command (:kbd:`⌘+Shift+X`).
   #. Search for ``ironplc``.
   #. Select :guilabel:`IronPLC IDE` in Visual Studio Code, or
      :guilabel:`IronPLC` in editors that install from Open VSX, then select
      :guilabel:`Install`.

   .. rubric:: Install IronPLC Extension from a VSIX

   If your development environment cannot reach either registry, install the
   extension manually:

   #. Download the latest IronPLC extension
      :download_artifact:`ironplc-vscode-extension.vsix` from
      `IronPLC GitHub releases`_.
   #. In the Extensions view, select :menuselection:`... (View and More Actions) --> Install from VSIX...` button.
   #. In the :guilabel:`Install from VSIX` dialog, select the VSIX file you downloaded earlier.

.. tab:: Linux

   .. rubric:: Install IronPLC CLI

   Open a terminal and run:

   .. code-block:: sh

      curl -fsSL https://www.ironplc.com/install.sh | sh

   This installs IronPLC into ``$HOME/.ironplc/bin`` and adds that
   directory to your ``PATH`` via your shell profile.

   To install a specific version:

   .. code-block:: sh

      curl -fsSL https://www.ironplc.com/install.sh | IRONPLC_VERSION=v0.201.0 sh

   Prebuilt binaries are currently provided for x86_64 Linux only.

   .. rubric:: Install IronPLC Extension

   Run your development environment, then:

   #. Go to the Extensions view by clicking on the Extensions icon in
      :guilabel:`Activity Bar` on the side of the window or using the
      View: Extensions command (:kbd:`Ctrl+Shift+X`).
   #. Search for ``ironplc``.
   #. Select :guilabel:`IronPLC IDE` in Visual Studio Code, or
      :guilabel:`IronPLC` in editors that install from Open VSX, then select
      :guilabel:`Install`.

   .. rubric:: Install IronPLC Extension from a VSIX

   If your development environment cannot reach either registry, install the
   extension manually:

   #. Download the latest IronPLC extension
      :download_artifact:`ironplc-vscode-extension.vsix` from
      `IronPLC GitHub releases`_.
   #. In the Extensions view, select :menuselection:`... (View and More Actions) --> Install from VSIX...` button.
   #. In the :guilabel:`Install from VSIX` dialog, select the VSIX file you downloaded earlier.

--------------------------------------
Next Steps
--------------------------------------

You are ready to start programming. In the next chapter, you will learn how
PLC programs work before writing your first one.

Continue to :doc:`sense-control-actuate`.

.. _IronPLC GitHub releases: https://github.com/ironplc/ironplc/releases/
