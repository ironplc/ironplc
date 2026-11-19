IronPLC selects between integers, reals, bit strings, ``BOOL``, times and
dates, enumerations, subranges, strings, structures and arrays. Every input is
evaluated, including the ones not selected. A selection of strings, structures
or arrays is supported where it is the whole value assigned, and each input is
a literal or a variable. Selecting inside an expression, such as an argument
of ``CONCAT``, or between inputs that call a function or use a computed array
index, is not yet supported.
