=========
Retention
=========

Retention qualifiers control whether variables preserve their values
across power cycles and program restarts.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.4.3
   * - **Support**
     - Not yet supported

Qualifiers
----------

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Qualifier
     - Description
   * - ``RETAIN``
     - Value is preserved across power cycles
   * - ``NON_RETAIN``
     - Value is reset to initial value on restart
   * - ``CONSTANT``
     - Value cannot be modified after initialization

Example
-------

.. code-block::

   PROGRAM main
       VAR RETAIN
           run_hours : DINT := 0;
       END_VAR
       VAR CONSTANT
           MAX_TEMP : INT := 150;
       END_VAR
       VAR
           current_temp : INT;
       END_VAR

       run_hours := run_hours + 1;
   END_PROGRAM

PERSISTENT
----------

.. include:: /includes/requires-dialect-extension.rst

The ``PERSISTENT`` qualifier marks variables whose values survive a
download and a reset as well as a power cycle. Enable it with the
``--allow-persistent-var`` flag; the ``codesys`` and ``twincat`` dialects
enable it.

.. list-table::
   :header-rows: 1
   :widths: 35 65

   * - Qualifier
     - Description
   * - ``PERSISTENT``
     - Value is preserved across downloads, resets and power cycles
   * - ``RETAIN PERSISTENT``
     - Same as ``PERSISTENT``
   * - ``PERSISTENT RETAIN``
     - Same as ``RETAIN PERSISTENT``: the two orders mean the same thing

A ``PERSISTENT`` qualifier may follow ``VAR`` and ``VAR_GLOBAL``, and the
``VAR_INPUT`` and ``VAR_OUTPUT`` of a function block or program. A function
keeps no values between calls, so a function that declares a ``PERSISTENT``
variable is an error (:doc:`/reference/compiler/problems/P4099`).

.. playground::
   :allows: persistent-var

   FUNCTION_BLOCK Counter
       VAR_INPUT PERSISTENT
           start : DINT := 0;
       END_VAR
       VAR_OUTPUT RETAIN PERSISTENT
           count : DINT;
       END_VAR
       count := count + 1;
   END_FUNCTION_BLOCK

   PROGRAM main
       VAR PERSISTENT
           run_hours : DINT := 0;
       END_VAR
       VAR
           counter : Counter;
       END_VAR
       counter();
       run_hours := counter.count;
   END_PROGRAM

See Also
--------

- :doc:`declarations` — basic variable syntax
- :doc:`initial-values` — initialization
