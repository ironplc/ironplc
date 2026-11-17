==============
Update IronPLC
==============

There is normally a new IronPLC release every Monday. There is no
automatic mechanism to distribute updates.

------------
Update Steps
------------

Follow the steps below to update IronPLC.

.. tab:: Windows

   .. rubric:: Update IronPLC CLI

   #. In search on the taskbar, enter :guilabel:`Control Panel` and select it from the results.
   #. Select :menuselection:`Programs --> Programs and Features`.
   #. Right-click (or press and hold) on :guilabel:`IronPLC` and select :guilabel:`Uninstall/Change`. Then follow the directions on the screen.
   #. Download the latest IronPLC installer from `IronPLC GitHub releases`_:

      * x64 :download_artifact:`ironplcc-x86_64-windows.exe`

      * Arm64 :download_artifact:`ironplcc-aarch64-windows.exe`
      
   #. Run the installer and follow the prompts to complete
      installation of the CLI.

.. tab:: macOS

   .. rubric:: Update IronPLC CLI

   If you installed IronPLC with Homebrew, open a Terminal and run:

   .. code-block:: sh

      brew update
      brew upgrade ironplc

   If you installed IronPLC with the install script, open a Terminal
   and re-run the script to install the latest version:

   .. code-block:: sh

      curl -fsSL https://www.ironplc.com/install.sh | sh

.. tab:: Linux

   .. rubric:: Update IronPLC CLI

   Open a terminal and re-run the install script to install the latest
   version:

   .. code-block:: sh

      curl -fsSL https://www.ironplc.com/install.sh | sh

.. rubric:: Update IronPLC Extension

Updating the extension is the same on every operating system.

.. include:: /includes/download-extension-vsix.rst

.. include:: /includes/install-extension-from-vsix.rst

.. _IronPLC GitHub releases: https://github.com/ironplc/ironplc/releases/
