# Contributing

This contributing guide tells you how to develop the docs.

## Prerequisites

You will need Git, Visual Studio Code and the Dev Containers
extension. If you're building outside the Dev Container you also need
Python 3 and `pip3` (used by `just setup` to install Sphinx and the
theme/extensions).

## Developing

1. Open the directory containing this file in Visual Studio Code.
1. In the Dev Container terminal, change to the `docs` folder.
1. Run `just setup` once to install Sphinx and the required extensions.
1. Run `just` to build the site.
1. Open `_build/index.html` in a browser.

`just ci` (which runs `setup` + `compile`) is what continuous integration
runs, and is useful for validating the full build from a clean state.
