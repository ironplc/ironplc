# Contributing

This component is the `ironplcc` compiler.

## Developing

Compilers and runtimes are tricky to get right and hard to keep right. Use
Cargo to run tests during development:

```sh
cargo test
```

### Running the Full Test Suite

The `Cargo` test approach does not execute all tests. The full test suite
is defined in GitHub actions workflow. You can run the full tests locally
using [act](https://github.com/nektos/act) (requires Docker).

Follow the steps described in the [act](https://github.com/nektos/act)
repository to install `act`.

```sh
act --workflows ./.github/workflows/commit.yaml
```

### Debugging the Parser

The PEG parser is difficult to debug without a little help. The steps below
will help enormously in understanding and fixing what the parser is doing.

Run tests with the `trace` feature enabled to get output on rule matching
for any test that is failing:

```sh
cargo test --features trace
```

For even better debug support, use pegviz. First, build and install the pegviz
application into your path.

```sh
git clone https://github.com/fasterthanlime/pegviz.git
cd pegviz
cargo install --force --path .
```

After installing pegviz, pipe output to pegviz for pretty printing of results,
then open the generated file in a web browser.

```sh
cargo test --features trace | pegviz --output ./pegviz.html
```

## How It Works

The project is split into 3 parts:

* `dsl` defines relevant domain objects from the IEC 61131-3 language; it is
   the intermediate set of objects from parsing and contains an abstract syntax
   tree as one component (among many)
* `parser` is tokenizes and parses an IEC 61131-3 text file into the `dsl`
   objects
* `plc2x` is the front-end for a source-to-source compiler; it assembles all
   the pieces

There is no strict definition of what goes where. Better rules would be nice.
