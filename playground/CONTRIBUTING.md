# Contributing

This contributing guide tells you how to develop the IronPLC interactive
playground. The playground is a browser-based editor and runner built from
the `compiler/playground` WASM crate plus static assets in this directory.

## Prerequisites

You will need:

* Git
* Rust and Cargo (stable)
* Node.js and npm
* Python 3 (used by `just serve` to host the build locally)

If you are using the Dev Container you already have these.

## Developing

1. In a terminal, change to this `playground` directory.
1. Run `just setup` once to install `wasm-pack`.
1. Run `just compile` to build the WASM package and assemble the site into
   `_build/`. This also runs `npm install` to pull in the JS dependencies.
1. Run `just serve` to build and serve the site at
   <http://localhost:8080> via Python's built-in HTTP server.

## Run Tests

End-to-end tests use Playwright:

```sh
just test
```

This installs the Playwright Chromium browser and runs the test suite
against a freshly compiled `_build/`.

## Before You Open a PR

Run the full CI recipe and make sure it passes:

```sh
just ci
```

This runs `setup` and then `test` (which depends on `compile`), matching
what GitHub Actions runs for this component.

## Clean Up

`just clean` removes the `_build/` directory.
