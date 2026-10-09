The return type is the type that every compared or selected input widens to.
The compiler converts a narrower input to that type first, so an ``INT`` and a
``LINT`` give a ``LINT``. When no input's type holds every other input, as with
a ``DINT`` and a ``UDINT``, the return type is the type of the first input that
is not a literal.
