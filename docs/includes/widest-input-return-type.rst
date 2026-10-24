The return type is the type that every compared or selected input widens to.
The compiler converts a narrower input to that type first, so an ``INT`` and a
``LINT`` give a ``LINT``. When no input's type holds every other input, as with
a ``DINT`` and a ``UDINT``, the call is an error
(:doc:`P4049 </reference/compiler/problems/P4049>`).
