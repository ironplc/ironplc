A ``$`` inside a literal starts an escape. Each escape is one character of
the string:

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Escape
     - Character
   * - ``$$``
     - Dollar sign
   * - ``$'``
     - Single quote
   * - ``$"``
     - Double quote
   * - ``$L`` or ``$N``
     - Line feed
   * - ``$R``
     - Carriage return
   * - ``$P``
     - Form feed
   * - ``$T``
     - Tab
   * - ``$`` and two hex digits (``STRING``)
     - The character with that code, for example ``$41`` is ``A``
   * - ``$`` and four hex digits (``WSTRING``)
     - The character with that code, for example ``$20AC`` is ``€``

The letters may be lower case. Any other escape is error :doc:`P0012 </reference/compiler/problems/P0012>`.
