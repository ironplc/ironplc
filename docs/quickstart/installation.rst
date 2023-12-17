.. _installation steps target:

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

   #. Download the latest IronPLC installer from `IronPLC GitHub releases`_:
   
      * x64 :download_artifact:`ironplcc-x86_64-windows.exe`

      * Arm64 :download_artifact:`ironplcc-aarch64-windows.exe`

   #. Run the installer and follow the prompts to complete
      installation of the CLI.

   **Install IronPLC Visual Studio Code Extension**

   Run Visual Studio Code, then in Visual Studio Code:

   #. Go to the Extensions view by clicking on the Extensions icon in
      :guilabel:`Activity Bar` on the side of VS Code or using the
      View: Extensions command (:kbd:`Ctrl+Shift+X`).
   #. In the Extensions view, enter :samp:`IronPLC` in the search box.
   #. In the Extensions view for the IronPLC item, choose :guilabel:`Install`.

.. tab:: macOS

   **Install IronPLC CLI**

   #. Go to `Homebrew <https://brew.sh/>`_ then follow the instructions to
      install Homebrew.
   #. In a Terminal, enter :program:`brew tap ironplc/tap`, then enter :program:`brew install ironplc`.

   **Install IronPLC Visual Studio Code Extension**

   Run Visual Studio Code, then in Visual Studio Code:

   #. Go to the Extensions view by clicking on the Extensions icon in
      :guilabel:`Activity Bar` on the side of VS Code or using the
      View: Extensions command (:kbd:`⌘+Shift+X`).
   #. In the Extensions view, enter :samp:`IronPLC` in the search box.
   #. In the Extensions view for the IronPLC item, choose :guilabel:`Install`.

.. tab:: Linux

   The weekly builds do include a homebrew tap for Linux but the tap is not
   tested other than to validate that it compiles.

.. _IronPLC GitHub releases: https://github.com/ironplc/ironplc/releases/
