=============================
Read Compiler Error Messages
=============================

When IronPLC finds a problem in your code, it produces a diagnostic message.
This guide explains how to read those messages and find the fix.

--------------------------------------
Anatomy of an Error Message
--------------------------------------

A typical error message from :program:`ironplcc check` looks like this:

.. code-block:: text

   error[P2001]: Variable 'Counter' is declared but type 'IN' is not defined
     --> main.st:3:7
      |
    3 |       Counter : IN := 0;
      |       ^^^^^^^ type 'IN' is not defined

The message has several parts:

- **error[P2001]** — the severity (error or warning) and the problem code.
  The code uniquely identifies the kind of problem.
- **Variable 'Counter' is declared but type 'IN' is not defined** — a
  human-readable summary of what went wrong.
- **--> main.st:3:7** — the file name, line number, and column where the
  problem was found.
- The **code snippet** shows the relevant line with a marker pointing to the
  exact location.

--------------------------------------
Using Problem Codes
--------------------------------------

Every problem code (like P2001 above) has a documentation page that
explains:

- When the error occurs
- An example that triggers it
- How to fix it

You can find all problem codes in the :doc:`/reference/compiler/problems/index`.

--------------------------------------
In the VS Code Extension
--------------------------------------

When you use the VS Code extension, errors and warnings appear as squiggly
underlines in the editor. To see the full message:

- Hover over the underlined code to see the diagnostic in a tooltip.
- Open the :guilabel:`Problems` panel (:menuselection:`View --> Problems`)
  to see all diagnostics for the workspace.
- Click on a diagnostic in the Problems panel to jump to the source location.

--------------------------------------
Common Mistakes
--------------------------------------

**Misspelled type name:**

.. code-block::

   VAR
      Counter : IN := 0;   (* should be INT *)
   END_VAR

**Missing semicolons:**

.. code-block::

   Counter := Counter + 1   (* missing ; *)

**Mismatched END keyword:**

.. code-block::

   PROGRAM main
   END_FUNCTION   (* should be END_PROGRAM *)

In each case, IronPLC points to the location of the problem and provides
a code you can look up for more detail.
