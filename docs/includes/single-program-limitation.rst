.. note::

   IronPLC currently runs one ``PROGRAM`` per configuration. Declaring a
   second ``PROGRAM``, or instantiating a program more than once across the
   ``RESOURCE`` blocks, is reported as :doc:`/reference/compiler/problems/P9999`
   at compile time. Support for more than one program is tracked in
   `issue #1613 <https://github.com/ironplc/ironplc/issues/1613>`_.
