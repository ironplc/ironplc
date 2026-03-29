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

   **Update IronPLC CLI**

   #. In search on the taskbar, enter :guilabel:`Control Panel` and select it from the results.
   #. Select :menuselection:`Programs --> Programs and Features`.
   #. Right-click (or press and hold) on :guilabel:`IronPLC` and select :guilabel:`Uninstall/Change`. Then follow the directions on the screen.
   #. Download the latest IronPLC installer from `IronPLC GitHub releases`_:

      * x64 :download_artifact:`ironplcc-x86_64-windows.exe`

      * Arm64 :download_artifact:`ironplcc-aarch64-windows.exe`
      
   #. Run the installer and follow the prompts to complete
      installation of the CLI.

   **Update IronPLC Extension**

   #. Download the latest IronPLC extension
      :download_artifact:`ironplc-vscode-extension.vsix` from
      `IronPLC GitHub releases`_.

   Run your development environment, then:

   #. Go to the Extensions view by clicking on the Extensions icon in
      :guilabel:`Activity Bar` on the side of the window or using the
      View: Extensions command (:kbd:`Ctrl+Shift+X`).
   #. In the Extensions view, select :menuselection:`... (View and More Actions) --> Install from VSIX...` button.
   #. In the :guilabel:`Install from VISX` dialog, select the VISX file you downloaded earlier.


.. tab:: macOS

   **Update IronPLC CLI**

   #. In a Terminal, enter :program:`brew update`, then enter :program:`brew upgrade ironplc`.

   **Update IronPLC Extension**

   #. Download the latest IronPLC extension
      :download_artifact:`ironplc-vscode-extension.vsix` from
      `IronPLC GitHub releases`_.

   Run your development environment, then:

   #. Go to the Extensions view by clicking on the Extensions icon in
      :guilabel:`Activity Bar` on the side of the window or using the
      View: Extensions command (:kbd:`⌘+Shift+X`).
   #. In the Extensions view, select :menuselection:`... (View and More Actions) --> Install from VSIX...` button.
   #. In the dialog, select the VISX file you downloaded earlier.


.. _IronPLC GitHub releases: https://github.com/ironplc/ironplc/releases/
