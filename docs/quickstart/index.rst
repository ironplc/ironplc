===========
Quick start
===========

Let's start your IronPLC journey! IronPLC is still expanding
so you journey will be quick. In this chapter, we'll discuss:

* Installing IronPLC
* Writing a program and checking syntax

IronPLC is supported on the following platforms:

- Windows (x64)

.. note::
   IronPLC is an early prototype that has lots of limitations. There are 
   weekly releases (labeled by date), and you should choose the latest release
   beause releases require passing a through test suite. That said, it is
   an early prototype, and there are lots of rough edges.

-------------
Prerequisites
-------------

The first step is to install Visual Studio Code. Go to
`Visual Studio Code <https://code.visualstudio.com/>`_ then follow the steps
for your platform.

------------
Installation
------------

Follow the steps below to install IronPLC.

.. tabs::
    .. tab:: Windows

        **Install IronPLC CLI**

        #. Download the latest IronPLC MSI installer,
           :file:`ironplc-release-windows.msi`, from `IronPLC GitHub releases`_.
        #. Run the MSI installer and follow the prompts to complete
           installation of the CLI.

        **Install IronPLC Visual Studio Code Extension**

        #. Download the latest IronPLC Visual Studio Code Extension,
           :file:`ironplc-vscode-extension-release.vsix`, from
           `IronPLC GitHub releases`_.

        Run Visual Studio Code, then in Visual Studio Code:

        #. Go to the Extensions view by clicking on the Extensions icon in
           :guilabel:`Activity Bar` on the side of VS Code or using the
           View: Extensions command (:kbd:`Ctrl+Shift+X`).
        #. In the Extensions view, select :menuselection:`... (View and More Actions) --> Install from VSIX...` button.
        #. In the :guilabel:`Install from VISX` dialog, select the VISX file you downloaded earlier.

    .. tab:: macOS

        Sorry, but not yet.

------------
Check a File
------------

IronPLC CLI includes an example that you can use to validate the installation.

Follow the steps below to check a file.

.. tabs::
    .. tab:: Windows

        #. In Visual Studio Code, select :menuselection:`File --> Open File...`.
        #. In the :guilabel:`Open File` dialog, select
           :file:`C:\Program Files\ironplc\examples\getting_started.st`
    
    .. tab:: macOS

        Sorry, but not yet.

.. _IronPLC GitHub releases: https://github.com/garretfick/ironplc/releases
