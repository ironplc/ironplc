==============
Tool Reference
==============

This chapter describes every tool the IronPLC MCP server exposes. Tools
are grouped by purpose: analysis tools validate source, context tools
summarise a project, and execution tools produce artifacts.

Every tool that takes source files shares two input fields:

``sources`` (array, required)
   One or more source files. Each entry has a ``name`` (logical file
   name — non-empty, at most 256 bytes of printable ASCII, unique
   within the array) and a ``content`` string with the full source
   text.

``options`` (object, required)
   Compiler options. Must include a ``dialect`` field set to one of the
   identifiers returned by :literal:`list_options`
   (for example, ``iec61131-3-ed2``, ``iec61131-3-ed3``, ``rusty``).
   May also include boolean feature-flag overrides
   (such as ``allow_c_style_comments``). Call :literal:`list_options`
   for the authoritative list.

All tools return ``diagnostics`` as an array of objects with ``code``,
``message``, ``file``, ``start``, ``end``, and ``severity`` fields.
``code`` is a problem code documented in
:doc:`/reference/compiler/problems/index`.

Analysis Tools
==============

list_options
------------

Enumerates the dialects and feature flags accepted in the ``options``
object of the source-accepting tools. Call this once at the start of a
session to learn what your build of :program:`ironplcmcp` supports;
the set of flags evolves with the compiler.

**Inputs:** none.

**Returns:** an object listing each dialect and each feature flag with
its option key and default value.

explain_diagnostic
------------------

Looks up the human-readable explanation for a compiler problem code.
Call this before editing code in response to a diagnostic you do not
fully understand — the explanation often names the exact construct
to change.

**Inputs:**

* ``code`` (string, required) --- Problem code to look up
  (e.g. ``"P0042"``). Case-insensitive; whitespace is trimmed.

**Returns:** an object with ``found``, ``code``, ``title``,
``description``, and an optional ``suggested_fix`` pulled from the
same documentation rendered at
:doc:`/reference/compiler/problems/index`.

parse
-----

Syntax check only. Confirms that the supplied sources tokenize and
parse. Use while drafting to catch missing keywords, unbalanced
brackets, and similar mistakes quickly. **Do not** use :literal:`parse`
to validate a change — it does not catch type errors, undeclared
symbols, or any other semantic rule. Call :literal:`check` for that.

**Inputs:** ``sources``, ``options``.

**Returns:** an object with ``ok``, a ``structure`` array listing the
top-level declarations found in each file, and ``diagnostics``.

check
-----

Primary validator. Runs the full pipeline through semantic analysis
and returns structured diagnostics. Always call :literal:`check`
before reporting a change as complete and before calling
:literal:`compile`. To self-heal, read the returned diagnostics, fix
the source, and call :literal:`check` again — use
:literal:`explain_diagnostic` to clarify any unfamiliar problem code
before editing.

**Inputs:** ``sources``, ``options``.

**Returns:** an object with ``ok`` and ``diagnostics``.

symbols
-------

Extracts the full symbol table for the supplied sources: programs,
functions, function blocks, their variables, and user-defined types.
Large responses are capped at 256 KiB; when the cap is reached the
response is empty with ``truncated`` set to ``true``. Use the
``pou`` filter or one of the context tools below when you only need
part of the answer.

**Inputs:**

* ``sources`` (required)
* ``options`` (required)
* ``pou`` (string, optional) --- Restrict the response to a single
  program, function, or function block by name. Matching is
  case-insensitive. Referenced user-defined types are still included.

**Returns:** an object with ``ok``, arrays ``programs``, ``functions``,
``function_blocks``, ``types``, a ``truncated`` flag, and
``diagnostics``. When ``pou`` is supplied, ``found`` indicates whether
a matching POU existed.

Context Tools
=============

project_manifest
----------------

Flat summary of what is declared across the supplied sources: file
names, program / function / function-block names, and user-defined
types grouped by kind (enumerations, structures, arrays, subranges,
aliases, strings, references). Use this to orient yourself in an
unfamiliar project before calling :literal:`symbols` or a more
targeted context tool.

**Inputs:** ``sources``, ``options``.

**Returns:** an object with ``ok``, ``files``, ``programs``,
``functions``, ``function_blocks``, grouped ``types``, and
``diagnostics``.

project_io
----------

Lists the inputs the caller can drive and the outputs the caller can
observe. This is the right tool to call before planning an execution
run — for example, to decide which variables to supply as stimuli or
which to trace.

**Inputs:** ``sources``, ``options``.

**Returns:** an object with ``ok``, ``inputs``, ``outputs``, and
``diagnostics``.

Execution Tools
===============

compile
-------

Runs the full pipeline (parse → semantic analysis → code generation)
and stores the resulting bytecode container in the server's in-memory
cache. Only call :literal:`compile` when you need a compiled artifact
— for validation, use :literal:`check`, which is faster and produces
the same diagnostics.

**Inputs:**

* ``sources`` (required)
* ``options`` (required)
* ``include_bytes`` (boolean, optional, default ``false``) --- When
  ``true``, the response includes the compiled container as a
  base64-encoded string.

**Returns:** an object with ``ok``, a ``container_id`` string for
later reference, an optional ``container_base64`` when
``include_bytes`` is ``true``, arrays ``tasks`` and ``programs``
describing the compiled configuration, and ``diagnostics``.

container_drop
--------------

Explicitly releases a compiled container from the server cache. Not
usually necessary — the cache evicts on LRU pressure automatically —
but useful for long-running connections that have finished with a
particular build.

**Inputs:**

* ``container_id`` (string, required) --- The identifier returned by
  a prior :literal:`compile` call.

**Returns:** an object with ``ok``, ``removed`` (``true`` when the
entry existed and was evicted), and ``diagnostics``.

See Also
========

* :doc:`overview` --- MCP server overview
* :doc:`ironplcmcp` --- Command-line reference
* :doc:`/reference/compiler/problems/index` --- Problem code index
  used by the :literal:`diagnostics` field and by
  :literal:`explain_diagnostic`
