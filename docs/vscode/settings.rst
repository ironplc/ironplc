==================
Settings Reference
==================

The IronPLC extension provides configuration settings to customize its behavior.
Access these settings through:

* File > Preferences > Settings (Windows/Linux)
* Code > Preferences > Settings (macOS)

Then search for "ironplc" to find all extension settings.

Available Settings
==================

ironplc.path
------------

:Type: String
:Default: Empty (auto-discovery)

Specifies the path to the ``ironplcc`` executable. When empty (the default), the
extension automatically searches for the compiler in standard locations.

Use this setting when:

* The compiler is installed in a non-standard location
* You want to use a specific version of the compiler
* Auto-discovery is not finding your installation

Example values:

* Windows: ``C:\Program Files\IronPLC\bin\ironplcc.exe``
* macOS: ``/usr/local/bin/ironplcc``
* Linux: ``/home/username/ironplc/ironplcc``

ironplc.logLevel
----------------

:Type: Enum
:Default: ``ERROR``
:Values: ``ERROR``, ``WARN``, ``INFO``, ``DEBUG``, ``TRACE``

Controls the verbosity of compiler logging. Higher levels include all messages from
lower levels.

* ``ERROR``: Only error messages (quietest)
* ``WARN``: Warnings and errors
* ``INFO``: Informational messages, warnings, and errors
* ``DEBUG``: Detailed debugging information
* ``TRACE``: Maximum verbosity (most detailed)

Increase the log level when troubleshooting issues with the extension or compiler.

ironplc.logFile
---------------

:Type: String
:Default: Empty (no file logging)

Specifies a file path where the compiler should write log messages. When empty,
logs are not written to a file.

This setting is useful for:

* Capturing detailed logs for bug reports
* Debugging issues that occur intermittently
* Analyzing compiler behavior over time

Example: ``/tmp/ironplc.log`` or ``C:\Users\username\ironplc.log``

.. note::

   The log file can grow large when using verbose log levels. Remember to disable
   file logging or delete the log file when troubleshooting is complete.

Settings in settings.json
=========================

You can also configure these settings directly in your ``settings.json`` file:

.. code-block:: json

   {
     "ironplc.path": "/custom/path/to/ironplcc",
     "ironplc.logLevel": "DEBUG",
     "ironplc.logFile": "/tmp/ironplc-debug.log"
   }

Workspace vs. User Settings
===========================

These settings can be configured at both the user level (applies to all projects)
and the workspace level (applies only to the current project).

Workspace settings are useful when:

* Different projects use different compiler versions
* A team wants to share consistent settings via version control
* You need project-specific logging for debugging
