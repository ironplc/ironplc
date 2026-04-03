==============
System Uptime
==============

Implicit global variables that expose the VM's monotonic uptime counter.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Not part of the standard (vendor extension)
   * - **Support**
     - Supported (requires ``--allow-system-uptime-global`` or ``--dialect rusty``)

Variables
---------

.. list-table::
   :header-rows: 1
   :widths: 30 15 55

   * - Variable
     - Type
     - Description
   * - ``__SYSTEM_UP_TIME``
     - ``TIME``
     - Milliseconds since VM start (wraps at ~24.8 days)
   * - ``__SYSTEM_UP_LTIME``
     - ``LTIME``
     - Milliseconds since VM start (effectively never wraps)

Description
-----------

``__SYSTEM_UP_TIME`` and ``__SYSTEM_UP_LTIME`` are implicit global variables
that the VM updates before each scan round. They contain the number of
milliseconds elapsed since the VM started, providing a monotonic uptime
counter.

Both variables are updated simultaneously and hold identical values within
the same scan round. All tasks in a scan round observe the same uptime.

``__SYSTEM_UP_TIME`` uses ``TIME`` (32-bit signed integer of milliseconds)
and wraps at approximately 24.8 days. Elapsed-duration subtraction
(``current - previous``) produces correct results as long as the interval
is under ~24.8 days, which covers all practical timer use cases.

``__SYSTEM_UP_LTIME`` uses ``LTIME`` (64-bit signed integer of milliseconds)
and effectively never wraps (~292 million years).

**Epoch and restart behavior:**

- Both variables start at 0 when the VM starts
- If the VM is stopped and restarted, the timer resets to 0

Enabling
--------

System uptime variables are a vendor extension and must be explicitly enabled:

.. code-block:: shell

   ironplcc check --allow-system-uptime-global main.st

Or use the RuSTy dialect which enables all vendor extensions:

.. code-block:: shell

   ironplcc check --dialect rusty main.st

See :doc:`/explanation/enabling-dialects-and-features` for more information
about dialects and feature flags.

Usage
-----

Access the system uptime variables using ``VAR_EXTERNAL``:

.. code-block:: text

   PROGRAM main
   VAR_EXTERNAL
       __SYSTEM_UP_TIME : TIME;
   END_VAR
   VAR
       elapsed : TIME;
   END_VAR
       elapsed := __SYSTEM_UP_TIME;
   END_PROGRAM

A common pattern is to wrap the variable in a function for compatibility
with CODESYS-style code:

.. code-block:: text

   FUNCTION TIME : TIME
   VAR_EXTERNAL
       __SYSTEM_UP_TIME : TIME;
   END_VAR
       TIME := __SYSTEM_UP_TIME;
   END_FUNCTION

See Also
--------

- :doc:`/explanation/enabling-dialects-and-features` -- enabling vendor extensions
