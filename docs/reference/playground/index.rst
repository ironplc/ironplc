====================
Playground Reference
====================

The `IronPLC playground <https://playground.ironplc.com>`_ compiles and runs
IEC 61131-3 programs entirely in the browser. It embeds the same compiler and
virtual machine as the command-line tools, compiled to WebAssembly, behind a
thin JavaScript host that loads programs, runs scan cycles, and reports results.

Most errors you see in the playground come from that shared compiler and VM and
carry the same codes documented elsewhere in this reference — a ``P####``
:doc:`compiler problem </reference/compiler/problems/index>` or a ``V####``
:doc:`runtime problem </reference/runtime/problems/index>`. A small number of
errors originate in the host layer itself — the WebAssembly wrapper that mediates
between the browser and the embedded compiler and VM. Those carry ``H####``
codes and are documented here.

.. toctree::
   :maxdepth: 1

   Problem Code Index <problems/index>
