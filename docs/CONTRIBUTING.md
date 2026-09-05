# Contributing

This contributing guide tells you how to develop the docs.

## Prerequisites

You will need Git, Visual Studio Code and the Dev Containers
extension. If you're building outside the Dev Container you also need
Python 3 and `pip3` (used by `just setup` to install Sphinx and the
theme/extensions), plus Node.js and `npm` (used by `just setup` to install
the duplication checker).

## Developing

1. Open the directory containing this file in Visual Studio Code.
1. In the Dev Container terminal, change to the `docs` folder.
1. Run `just setup` once to install Sphinx and the required extensions.
1. Run `just` to build the site.
1. Open `_build/index.html` in a browser.

`just ci` (which runs `setup` + `compile` + `duplicates`) is what continuous
integration runs, and is useful for validating the full build from a clean
state.

## Avoiding duplicated content

Text that belongs on more than one page lives once in `docs/includes/` and is
pulled in with `.. include::`. Use a substitution when the shared text varies
by a word or two, and a `:doc:`/`:ref:` cross-reference when the reader should
be sent to the authoritative page instead. See
[Avoid Duplication](../specs/steering/development-standards.md#avoid-duplication)
for the rule and for the kinds of duplication that are acceptable.

`just duplicates` runs [jscpd](https://jscpd.dev) over the `.rst` sources and
fails when duplication rises above the `threshold` in `.jscpd.json`.

**Read the threshold with care.** It is currently 19%, which is not a quality
target — it is roughly where the docs sit today. jscpd has no reStructuredText
tokenizer: it registers a `rest` format with no extension mapping and no
grammar, so RST falls through to a generic lexer that treats `//` and `/* */`
as comments. The result is that most of what it reports is section underlines
and `list-table` markup rather than duplicated prose, and the percentage
cannot be driven down to a meaningful number.

Two consequences:

- The check catches a **regression** in the aggregate figure, not individual
  duplicated paragraphs. When it fails, read `_duplication/jscpd-report.json`
  and judge the clones yourself; many will be markup.
- `reference/standard-library/` and `reference/compatibility-libraries/` are
  excluded in `.jscpd.json`. Those trees are full of parallel sibling pages
  whose symmetric wording is intentional, and leaving them in drowned
  everything else.

Fixing this properly means contributing an RST tokenizer to jscpd. That work
is tracked in [issue #1409](https://github.com/ironplc/ironplc/issues/1409).
