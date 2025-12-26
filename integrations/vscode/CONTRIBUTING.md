# Contributing

This contributing guide tells you how to develop changes to the
IronPLC Visual Studio Code Extension.

## Prerequisites

You will need Git, Node.js, NPM and Visual Studio Code. Install those using
your preferred source.

You will also need `ironplcc`. Install from your preferred source (in a later
step, you will to a custom-build version).

## Developing

1. Open the directory containing this file in Visual Studio Code
1. Run `just setup` to install dependencies (or `npm install`)
1. Run `just compile` to build the extension (or `npm run compile`)
1. Load the extension in a new Visual Studio Code by pressing `F5`. A new
   window opens labeled **[Extension Development Host]** with the running
   extension.
1. Make changes as desired.
1. Reload the extension by pressing `Ctrl+R` or `Cmd+R` on Mac.

## Use Custom IronPLC

You will frequently need to make changes to both this Visual Studio Code
Extension and IronPLC.

1. Build a local version of `ironplcc`.
1. In the **[Extension Development Host]** window, select **File » Preferences » Settings**.
1. In the **Settings** document, search for **ironplc**.
1. Set the value of **Ironplc: Path** to the directory containing `ironplcc`.

## Run Tests

1. Open the directory containing this file in Visual Studio Code.
1. Run `just test` to execute the functional tests (or use the debug viewlet as described below).

### Alternative: Debug Tests in VS Code
1. Open the debug viewlet (`Ctrl+Shift+D` or `Cmd+Shift+D` on Mac) and from the launch configuration dropdown pick **Extension Tests**.
1. Press `F5` to run the tests in a new window with your extension loaded.
