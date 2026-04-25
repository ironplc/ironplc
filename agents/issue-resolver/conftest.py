"""pytest rootdir marker for the issue-resolver package.

Having a ``conftest.py`` at this level tells pytest to add this
directory to ``sys.path`` so tests can import top-level modules
(``config``, ``ledger``, ``orchestrator``, …) directly. The folder
name contains a hyphen, so it cannot be a Python package itself —
this is the simplest way to give tests and runtime code a shared
import root.
"""
