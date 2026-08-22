==================
Settings Reference
==================

The IronPLC extension provides configuration settings to customize its behavior.
Access these settings through:

* :menuselection:`File --> Preferences --> Settings` (Windows/Linux)
* :menuselection:`Code --> Preferences --> Settings` (macOS)

Then search for "ironplc" to find all extension settings.

.. figure:: /images/screenshots/settings-panel.png
   :alt: VS Code settings panel filtered to show IronPLC extension settings
   :width: 600px

   The VS Code settings panel with IronPLC settings.

Available Settings
==================

ironplc.path
------------

:Type: String
:Default: Empty (auto-discovery)

Specifies the path to the :program:`ironplcc` executable. When empty (the default), the
extension automatically searches for the compiler in standard locations.

Use this setting when:

* The compiler is installed in a non-standard location
* You want to use a specific version of the compiler
* Auto-discovery is not finding your installation

Example values:

* Windows: ``C:\Program Files\IronPLC\bin\ironplcc.exe``
* macOS: ``/usr/local/bin/ironplcc``
* Linux: ``/home/username/ironplc/ironplcc``

ironplc.debugServerPath
-----------------------

:Type: String
:Default: Empty (auto-discovery)

Specifies the path to the :program:`ironplcvmd` debug server. When empty (the
default), the extension looks for the server next to the :program:`ironplcc`
compiler it discovered.

Use this setting when the debug server is installed apart from the compiler,
or when :doc:`problems/E0007` reports that the server was not found.

Example values:

* Windows: ``C:\Program Files\IronPLC\bin\ironplcvmd.exe``
* macOS: ``/usr/local/bin/ironplcvmd``
* Linux: ``/home/username/ironplc/ironplcvmd``

See :doc:`debugging` for what the debug server does.

.. note::

   This setting was named ``ironplc.dapServerPath`` in earlier releases, when
   the debug server was named :program:`ironplcdap`. The old name is no longer
   read; move any existing value to ``ironplc.debugServerPath``.

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

ironplc.dialect
---------------

:Type: Enum
:Default: ``iec61131-3-ed2``
:Values: ``iec61131-3-ed2``, ``iec61131-3-ed3``, ``rusty``, ``codesys``, ``twincat``

Selects the language dialect preset. A dialect controls the IEC 61131-3 edition
and a default set of extensions.

* ``iec61131-3-ed2``: Strict IEC 61131-3:2003 (Edition 2). No extensions.
* ``iec61131-3-ed3``: IEC 61131-3:2013 (Edition 3) with ``LTIME``, ``REF_TO``, etc.
* ``rusty``: RuSTy-compatible — designed for compatibility with code from
  RuSTy-based PLC environments.
* ``codesys``: CODESYS-compatible — Edition 2 base with ``REF_TO`` and the
  extensions that the CODESYS IDE accepts.
* ``twincat``: TwinCAT-compatible — Edition 2 base with the extensions
  Beckhoff TwinCAT shares with CODESYS. Unlike ``codesys`` it does not enable
  the ``REF_TO`` reference extensions, since TwinCAT uses ``REFERENCE TO``
  (bound with ``REF=``) and ``POINTER TO`` (bound with ``ADR()``), which it
  enables instead.

This setting corresponds to the ``--dialect`` command-line option documented in
:doc:`/reference/compiler/ironplcc`.

See :doc:`/explanation/enabling-dialects-and-features` for background on dialects and
:doc:`/reference/language/edition-support` for the full list of edition-gated
features.

Settings in settings.json
=========================

You can also configure these settings directly in your :file:`settings.json` file:

.. code-block:: json

   {
     "ironplc.path": "/custom/path/to/ironplcc",
     "ironplc.debugServerPath": "/custom/path/to/ironplcvmd",
     "ironplc.logLevel": "DEBUG",
     "ironplc.logFile": "/tmp/ironplc-debug.log",
     "ironplc.dialect": "rusty"
   }
