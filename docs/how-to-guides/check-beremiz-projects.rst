========================
Check Beremiz Projects
========================

This guide shows how to use IronPLC to check a Beremiz project for correctness.

.. include:: ../includes/requires-compiler.rst

-----------------------------------
Check with the VS Code Extension
-----------------------------------

Open your Beremiz project folder in VS Code. The IronPLC extension automatically
recognizes :file:`plc.xml` as a PLCopen XML file and highlights errors and warnings
in the editor.

-------------------------------------------
Check with the Command Line
-------------------------------------------

You can check the :file:`plc.xml` file directly:

.. code-block:: shell

   ironplcc check plc.xml

Or point IronPLC at the project directory. IronPLC detects Beremiz projects by
the presence of :file:`plc.xml` and loads it automatically:

.. code-block:: shell

   ironplcc check path/to/my-beremiz-project

See :doc:`/reference/compiler/source-formats/plcopen-xml` for complete details on PLCopen
XML format support.
