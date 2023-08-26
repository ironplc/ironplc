============
Installation
============

IronPLC is supported on the following platforms:

- Windows (x64)
- macOS

-------------
Prerequisites
-------------

The first step is to install Visual Studio Code. Go to
`Visual Studio Code <https://code.visualstudio.com/>`_ then follow the steps
for your platform.

-------------
Install Steps
-------------

Follow the steps below to install IronPLC.

.. tab:: Windows

   **Install IronPLC CLI**

   #. Download the latest IronPLC MSI installer :download_artifact:`ironplcc-x86_64-windows.msi`
      from `IronPLC GitHub releases`_.
   #. Run the MSI installer and follow the prompts to complete
      installation of the CLI.

   **Install IronPLC Visual Studio Code Extension**

   #. Download the latest IronPLC Visual Studio Code Extension
      :download_artifact:`ironplc-vscode-extension.vsix` from
      `IronPLC GitHub releases`_.

   Run Visual Studio Code, then in Visual Studio Code:

   #. Go to the Extensions view by clicking on the Extensions icon in
      :guilabel:`Activity Bar` on the side of VS Code or using the
      View: Extensions command (:kbd:`Ctrl+Shift+X`).
   #. In the Extensions view, select :menuselection:`... (View and More Actions) --> Install from VSIX...` button.
   #. In the :guilabel:`Install from VISX` dialog, select the VISX file you downloaded earlier.

.. tab:: macOS

   **Install IronPLC CLI**

   #. Go to `Homebrew <https://brew.sh/>`_ then follow the instructions to
      install Homebrew.
   #. In a Terminal, enter :program:`brew tap ironplc/tap`, then enter :program:`brew install ironplc`.

   **Install IronPLC Visual Studio Code Extension**

   #. Download the latest IronPLC Visual Studio Code Extension
      :download_artifact:`ironplc-vscode-extension.vsix` from
      `IronPLC GitHub releases`_.

   Run Visual Studio Code, then in Visual Studio Code:

   #. Go to the Extensions view by clicking on the Extensions icon in
      :guilabel:`Activity Bar` on the side of VS Code or using the
      View: Extensions command (:kbd:`⌘+Shift+X`).
   #. In the Extensions view, select :menuselection:`... (View and More Actions) --> Install from VSIX...` button.
   #. In the dialog, select the VISX file you downloaded earlier.

.. tab:: Linux

   The weekly builds do include a homebrew tap for Linux but the tap is not
   tested other than to validate that it compiles.

.. _IronPLC GitHub releases: https://github.com/ironplc/ironplc/releases/
