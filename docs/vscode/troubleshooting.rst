===============
Troubleshooting
===============

This guide helps you resolve common issues with the IronPLC extension.

No Syntax Highlighting
======================

**Symptom**: Files appear as plain text without colors.

**Solutions**:

1. **Check file extension**: Ensure your file has a supported extension:

   * Structured Text: :file:`.st`, :file:`.iec`
   * TwinCAT: :file:`.TcPOU`, :file:`.TcGVL`, :file:`.TcDUT`
   * PLCopen XML: detected automatically by XML namespace

2. **Check language mode**: Look at the bottom-right corner of VS Code. It should
   show "IEC 61131-3", "Structured Text", "TwinCAT POU", or "PLCopen XML".
   Click it to change the language mode if needed.

3. **Reload the window**: Run "Developer: Reload Window" from the Command Palette.

No Diagnostics Appearing
========================

**Symptom**: The extension does not show any errors or warnings, even for invalid code.

**Solutions**:

1. **Check the compiler is running**: Look at the Output panel (:menuselection:`View --> Output`) and
   select "IronPLC" from the dropdown. You should see startup messages.

2. **Check for E0001**: If the compiler was not found, diagnostics will not work.
   See :doc:`problems/E0001`.

3. **Enable debug logging**: Set ``ironplc.logLevel`` to ``DEBUG`` and check the
   Output panel for error messages.

Extension Not Activating
========================

**Symptom**: The extension appears installed but nothing happens when opening ST files.

**Solutions**:

1. **Check extension status**: Open the Extensions view and find IronPLC. Ensure
   it shows as enabled, not disabled.

2. **Check for conflicts**: Disable other IEC 61131-3 or PLC extensions that might
   conflict.

3. **View extension logs**: Open :menuselection:`Help --> Toggle Developer Tools`, go to the Console
   tab, and look for messages containing "ironplc".

4. **Reinstall the extension**: Uninstall the extension, reload VS Code, then
   reinstall it.

Collecting Debug Information
============================

When reporting issues, please collect the following information:

1. **Version information**:

   * VS Code version (:menuselection:`Help --> About`)
   * IronPLC extension version (Extensions view)
   * Compiler version (run :program:`ironplcc --version`)
   * Operating system and version

2. **Enable detailed logging**:

   .. code-block:: json

      {
        "ironplc.logLevel": "TRACE",
        "ironplc.logFile": "/tmp/ironplc-debug.log"
      }

3. **Reproduce the issue** with logging enabled.

4. **Collect logs**:

   * The log file specified above
   * Output panel content (:menuselection:`View --> Output --> IronPLC`)
   * Developer Tools console (:menuselection:`Help --> Toggle Developer Tools`)

5. **Create an issue** on `GitHub <https://github.com/ironplc/ironplc/issues>`_
   with:

   * Steps to reproduce the problem
   * Expected vs. actual behavior
   * Version information
   * Relevant log excerpts

Performance Issues
==================

**Symptom**: The editor is slow or unresponsive when editing ST files.

**Solutions**:

1. **Check file size**: Very large files may cause slowdowns. Consider splitting
   large programs into multiple files.

2. **Reduce log level**: If you have verbose logging enabled, set ``ironplc.logLevel``
   back to ``ERROR``.

3. **Disable file logging**: Remove any ``ironplc.logFile`` setting.

4. **Check for loops**: The compiler may take longer on code with complex nested
   structures. Simplify where possible.

Resetting the Extension
=======================

If all else fails, try a clean reset:

1. Uninstall the IronPLC extension
2. Delete the extension's global storage:

   * Windows: ``%USERPROFILE%\.vscode\extensions\ironplc.*``
   * macOS/Linux: ``~/.vscode/extensions/ironplc.*``

3. Reload VS Code
4. Reinstall the extension from the marketplace
