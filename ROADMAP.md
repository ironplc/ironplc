# Roadmap

This roadmap describes what IronPLC 1.0 means and what stands between today
and 1.0. It does not list supported features; the
[documentation website](https://www.ironplc.com) does that.

## The goal of 1.0

**IronPLC 1.0 means you can trust its results for IEC 61131-3 Edition 2.**

Trust is measured against public test cases. A program from the
[1.0 test corpus](TODO-link-to-corpus-design-doc) must produce the same
observed values in the variables it declares under IronPLC as the test case
expects. Where the standard leaves a behavior to the implementer, such as
floating-point arithmetic, IronPLC makes that behavior an explicit
[behavior policy](specs/steering/glossary.md#behavior-policy), so the result
is deterministic and the same on every platform, whichever compiler the test
case came from.

The Edition 2 bar is not negotiable. Everything else is judged on its quality
at the time the Edition 2 core is trusted.

IronPLC is not limited to Edition 2. It already supports Edition 3 features,
including object-oriented programming and references, as well as language
extensions and dialects for CODESYS, Beckhoff TwinCAT and RuSTy. These work
today and you can use them. They are just not what decides when IronPLC
reaches 1.0.

## Where things are now

The current focus is fixing defects: making IronPLC's observed values match
the corpus, and making implementer-defined behavior into policies.

## Required for 1.0

- Every program in the 1.0 test corpus produces the same observed values.
- Floating-point behavior is controlled by behavior policies.
- Time is represented and controlled by behavior policies.
- There are no known defects where IronPLC compiles and runs a program but
  produces a wrong value.

## Included in 1.0 if ready

Most of these are already available today. They become part of the 1.0
promise if they are as trustworthy as the Edition 2 core when 1.0 is ready.
They do not delay 1.0. Anything not ready stays available behind its
`--allow-*` flag or dialect, without the promise, until a later release.

- Edition 3 features, including object-oriented programming and references
  (`REF_TO`, `REF`) (available)
- Language extensions and the CODESYS, TwinCAT and RuSTy dialects (available)
- I/O binding (not yet available)

## What 1.0 promises

- **The command line is stable.** Commands and options that work in 1.0 keep
  working across 1.x.
- **Results are stable.** The same source, compiled with the same dialect and
  policy selections, produces the same observed values on every 1.x release.
  The promise covers the default Edition 2 dialect and anything the release
  notes declare ready. The one exception is a defect fix: if IronPLC produces
  a wrong value, a 1.x release may correct it.
- **Bytecode is safe but not stable.** A compiled `.iplc` file may not load
  in a different version of the runtime. Any change that alters how bytecode
  is read creates a new format revision, and the runtime rejects a file with
  a revision it does not support rather than misreading it. Recompile after
  you upgrade.

## What 1.0 does not promise

- **Code-level integration.** The Rust crates that make up IronPLC have no
  stability guarantee. Projects that build on them should pin an exact
  version.
- **Features behind a flag or a non-default dialect,** unless the release
  notes declare them ready.
- **Performance.** Performance is a 2.0 concern.

## Contributing

Contributions are welcome, including toward anything under
[Included in 1.0 if ready](#included-in-10-if-ready). A contribution can land
behind a flag at any time, and it can join the 1.0 promise if it meets the
same bar as the Edition 2 core. Contributions do not change the 1.0 criteria.
See [CONTRIBUTING.md](CONTRIBUTING.md) to get started.
