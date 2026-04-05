.. _installation steps target:

============
Installation
============

IronPLC supports the following platforms:

- Windows (x64)
- macOS

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

   **Install IronPLC CLI**

   #. Download the latest IronPLC installer from `IronPLC GitHub releases`_:
   
      * x64 :download_artifact:`ironplcc-x86_64-windows.exe`

      * Arm64 :download_artifact:`ironplcc-aarch64-windows.exe`

   #. Run the installer and follow the prompts to complete
      installation of the CLI.

   **Install IronPLC Extension**

   Run your development environment, then:

   #. Go to the Extensions view by clicking on the Extensions icon in
      :guilabel:`Activity Bar` on the side of the window or using the
      View: Extensions command (:kbd:`Ctrl+Shift+X`).
   #. In the Extensions view, enter :samp:`IronPLC` in the search box.
   #. In the Extensions view for the IronPLC item, choose :guilabel:`Install`.

.. tab:: macOS

   **Install IronPLC CLI**

   #. Go to `Homebrew <https://brew.sh/>`_ then follow the instructions to
      install Homebrew.
   #. In a Terminal, enter :program:`brew tap ironplc/tap`, then enter :program:`brew install ironplc`.

   **Install IronPLC Extension**

   Run your development environment, then:

   #. Go to the Extensions view by clicking on the Extensions icon in
      :guilabel:`Activity Bar` on the side of the window or using the
      View: Extensions command (:kbd:`⌘+Shift+X`).
   #. In the Extensions view, enter :samp:`IronPLC` in the search box.
   #. In the Extensions view for the IronPLC item, choose :guilabel:`Install`.

.. tab:: Linux

   The weekly builds do include a homebrew tap for Linux but the tap is not
   tested other than to validate that it compiles.

--------------------------------------
Next Steps
--------------------------------------

You are ready to start programming. In the next chapter, you will learn how
PLC programs work before writing your first one.

Continue to :doc:`sense-control-actuate`.

.. _IronPLC GitHub releases: https://github.com/ironplc/ironplc/releases/
