IEC 61131-3 declares the inputs as any elementary type (``ANY_ELEMENTARY``).
IronPLC compares integers, reals, bit strings, ``BOOL``, times and dates, and
strings: ``FALSE`` is less than ``TRUE``, a time or date is ordered by the
duration or instant it stands for, and strings are ordered as ``<`` orders
them. An enumeration, a structure or an array is not elementary, and is an
error (:doc:`P4026 </reference/compiler/problems/P4026>`). Every input is
evaluated. Of strings, the function is supported where it is the whole value
assigned, and each input is a literal or a variable. Using it inside an
expression, such as an argument of ``CONCAT``, or with inputs that call a
function or use a computed array index, is not yet supported.
