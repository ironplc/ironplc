.. image:: images/banner.svg
   :align: center

IronPLC is a software development environment for building industrial control systems using off-the-shelf computers. More precisely, IronPLC will one day be an integrated development environment for building IEC 61131-3 based control systems that run on off-the-shelf embedded computers. In effect, we want to make it easy to build SoftPLCs.

The goal is ambitious and IronPLC is far from achieving that goal.
Today, IronPLC
integrates into Visual Studio Code and provides tools to work with IEC 61131-3 files:

* auto-completion
* syntax and semantic checks

IronPLC has some pretty big limitations and is not going to replace your
existing development environment any time soon. Even so, we'd love if you give
it a try, let us know what you think, fix a bug, or even become a regular
contributor. IronPLC is MIT licensed and we aim to keep it that way.

.. grid:: 2

    .. grid-item-card::  Tutorials

        Start here as a new developer:

        * :doc:`quickstart/index`

    .. grid-item-card::  How-to guides

        Step-by-step guides for using IronPLC.

        * :ref:`how to guides target`
   
.. grid:: 2

   .. grid-item-card::  Reference

        Technical reference material, for
        
        * :doc:`compiler/index`
        * :doc:`vscode/index`
        * :doc:`developer/index`

.. toctree::
   :maxdepth: 1
   :hidden:

   Quick start <quickstart/index>
   How-to guides <how-to-guides/index>
   Compiler reference <compiler/index>
   Visual Studio Code extension reference <vscode/index>
   Developer documentation <developer/index>
