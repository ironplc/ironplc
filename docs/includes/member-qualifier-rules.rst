- A qualifier appears at most once, and there is at most one access
  specifier (``PUBLIC``, ``PRIVATE``, ``PROTECTED``, ``INTERNAL``).
- The access specifier comes first, before ``FINAL``, ``ABSTRACT`` and
  ``OVERRIDE``. ``METHOD FINAL PUBLIC M`` is an error; ``METHOD PUBLIC
  FINAL M`` is not.
- ``FINAL`` and ``ABSTRACT`` exclude each other.
- A function block cannot be ``PRIVATE``, ``PROTECTED`` or ``OVERRIDE``.
- An ``ABSTRACT`` method has no body, and only an ``ABSTRACT`` function
  block can declare one.
