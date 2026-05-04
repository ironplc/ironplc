=======================
System Clock and Uptime
=======================

Many programs need to know the current date/time (e.g. January 2, 2025 at 14:50 UTC)
and/or how long the program has been running (e.g. 2 days, 3 hours, 22 minutes, ...).

Time is an essential part of PLC programming. After all, PLC programs
run on a timed scan cycle and IEC 61131-3 defines multiple data types to represent
dates, times, and durations. However, IEC 61131-3 does not define how to get access
to either the system clock or the system uptime. Therefore, vendors offer different
mechanisms for programs to query the system clock and update.

-----------------------
System Clock on IronPLC
-----------------------

This section is still TODO.

------------------------
System Uptime on IronPLC
------------------------

When enabled, IronPLC updates the system update into two variables each scan cycle and before
any task runs. The default name of these variables  are ``__SYSTEM_UP_TIME`` and
``__SYSTEM_UP_LTIME``.

These variables are specific to IronPLC so you must enable this feature as a vendor extension
using ``--allow-system-uptime-global`` or a dialect that enables this feature. See
:doc:`/explanation/enabling-dialects-and-features` for more information about dialects and feature flags.

When enabled, you can write programs that directly reference these variables as external variables,
for example:

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

--------------------------
Vendor Compatibility Shims
--------------------------

Code written for other vendor environments typically need a shim so that the code will compile
with IronPLC. The shim emulates the POU(s) provided by the vendors library.

.. rubric:: Codesys

The ``TIME`` function in Codesys returns the system uptime. Follow the steps
below to configure IronPLC with equivalent behavior:

#. Define a function named ``TIME`` as follows:

    .. code-block:: shell

       FUNCTION TIME : TIME
          TIME := __SYSTEM_UP_TIME;
       END_FUNCTION

#. Configure IronPLC to use the Codesys dialect. See :doc:`/explanation/enabling-dialects-and-features`
   for how to configure the dialect.

.. rubric:: RuSTy

The ``TIME`` function in RuSTy returns the system uptime. Follow the steps
below to configure IronPLC with similar behavior:

#. Define a function named ``TIME`` as follows:

    .. code-block:: shell

       FUNCTION TIME : TIME
          TIME := __SYSTEM_UP_TIME;
       END_FUNCTION

#. Configure IronPLC to use the RuSTy dialect. See :doc:`/explanation/enabling-dialects-and-features`
   for how to configure the dialect.

-----------
Clock Drift
-----------

This section is still TODO.

--------
See Also
--------

:doc:`/explanation/enabling-dialects-and-features`
:doc:`/reference/extension-library/variables/system-uptime`
