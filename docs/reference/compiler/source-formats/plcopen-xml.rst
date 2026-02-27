=============
PLCopen XML
=============

IronPLC supports the PLCopen XML interchange format (TC6 XML) for importing
IEC 61131-3 programs from other development environments. The supported version
is **PLCopen TC6 XML version 2.01** (namespace: ``http://www.plcopen.org/xml/tc6_0201``).

---------------
File Extensions
---------------

PLCopen XML files use the ``.xml`` extension. IronPLC detects PLCopen XML files
by the presence of the TC6 XML namespace, not by file extension alone.

-------------------
Supported Languages
-------------------

.. include:: ../../../includes/supported-languages.rst

------------------
Supported Elements
------------------

.. include:: ../../../includes/supported-elements.rst

-----------------
Project Discovery
-----------------

When you point IronPLC at a directory, it automatically detects Beremiz projects
by the presence of a :file:`plc.xml` file. If found, IronPLC loads :file:`plc.xml`
as a PLCopen XML file.
