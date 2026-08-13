============
Tc2_BuiltIns
============

Compatibility library for the operators that TwinCAT 3 provides to every
PLC project *without* any library reference. Because the vendor
environment treats these as implicitly available, IronPLC activates this
library automatically whenever it discovers a TwinCAT project — no
project reference or ``--library`` option is needed.

Functions
---------

.. list-table::
   :header-rows: 1
   :widths: 25 30 45

   * - Name
     - Signature
     - Description
   * - ``BOOL_TO_STRING``
     - ``BOOL_TO_STRING(IN : BOOL) : STRING``
     - Converts a boolean to its text form: ``TRUE`` returns
       ``'TRUE'``, ``FALSE`` returns ``'FALSE'``.

Example
-------

.. code-block::

   statusText := BOOL_TO_STRING(running);

Independence
------------

.. include:: ../../includes/compat-library-independence.rst

See Also
--------

* :doc:`index` — how compatibility libraries are activated
* :doc:`/reference/standard-library/functions/type-conversions` — the IEC
  standard conversion functions
* `Beckhoff TwinCAT 3: type conversion operators <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/3998090635.html>`_
  — the vendor documentation this interface reproduces
