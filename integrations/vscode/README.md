# IronPLC Visual Studio Code Extension

IronPLC brings IEC 61131-3 support to Visual Studio Code.

⚠ This project's capabilities are limited to a parser, semantic analyzer, and
Visual Studio Code Extension that are building blocks for a complete
IEC 61131-3 runtime and development environment.

## Quick Start

* **Step 1.** [Install the IronPLC compiler and Visual Studio Code extension](https://www.ironplc.com/quickstart/installation.html)
on your system.
* **Step 2.** Open or create an IEC 61131-3 file and start coding.

## Supported File Types

| **File Type** | **Extensions** | **Description** |
|---------------|----------------|-----------------|
| Structured Text | `.st`, `.iec` | IEC 61131-3 Structured Text source files |
| PLCopen XML | Auto-detected | PLCopen TC6 XML project files with embedded Structured Text |

## Features

### Syntax Highlighting

Full syntax highlighting for IEC 61131-3 Structured Text including:

* Keywords (PROGRAM, FUNCTION, FUNCTION_BLOCK, VAR, IF, FOR, WHILE, etc.)
* Data types (BOOL, INT, REAL, STRING, TIME, etc.)
* Operators and literals
* Comments (`(* block comments *)`)

PLCopen XML files are also highlighted with embedded Structured Text support.

### Real-Time Analysis

As you type, IronPLC analyzes your code and reports:

* Syntax errors
* Semantic errors (type mismatches, undefined variables, etc.)
* Warnings for potential issues

Diagnostics appear inline and in the Problems panel.

### Auto-Closing Pairs

The extension automatically closes:

* **Brackets and strings**: `[]`, `()`, `''`, `""`
* **Comments**: `(* *)`
* **Control structures**: `IF`/`END_IF`, `FOR`/`END_FOR`, `WHILE`/`END_WHILE`, `CASE`/`END_CASE`, `REPEAT`/`END_REPEAT`
* **Program units**: `PROGRAM`/`END_PROGRAM`, `FUNCTION`/`END_FUNCTION`, `FUNCTION_BLOCK`/`END_FUNCTION_BLOCK`
* **Variable blocks**: `VAR`/`END_VAR`, `VAR_INPUT`/`END_VAR`, `VAR_OUTPUT`/`END_VAR`, and other VAR types
* **Other blocks**: `STRUCT`/`END_STRUCT`, `ACTION`/`END_ACTION`, `CONFIGURATION`/`END_CONFIGURATION`

### Bracket Colorization

Matching brackets are colorized to help visualize nesting levels.

## Commands

Open the Command Palette (Command+Shift+P on macOS and Ctrl+Shift+P
on Windows) and type in one of the following commands:

| **Command** | **Description** |
|-------------|-----------------|
| `IronPLC: New Structured Text File` | Create a new IEC 61131-3 structured text file |

## Extension Settings

| **Setting** | **Description** | **Default** |
|-------------|-----------------|-------------|
| `ironplc.path` | Path to the IronPLC compiler executable. If empty, discovers based on PATH. | (empty) |
| `ironplc.logLevel` | Log level for the compiler: ERROR, WARN, INFO, DEBUG, or TRACE | ERROR |
| `ironplc.logFile` | Path to write compiler logs. If empty, logs are not written to file. | (empty) |

## Platform Support

The extension works on:

* Windows
* macOS
* Linux

## Learn More

Visit [ironplc.com](https://www.ironplc.com) for documentation, tutorials, and guides.
