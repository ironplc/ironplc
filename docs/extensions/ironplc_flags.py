'''
Validates that all --allow-* compiler flags and every Dialect are documented.

This extension reads the source of truth (compiler/parser/src/options.rs),
extracts all allow_* CLI flags and every Dialect::cli_name(), and verifies each
one appears in the relevant doc pages.

The build fails if any flag or dialect is missing from its doc file(s), ensuring
documentation stays in sync when new flags or dialects are added.
'''
import re
from pathlib import Path
from sys import exit


def _options_text(app):
    """Read the compiler options source of truth (options.rs)."""
    srcdir = Path(app.srcdir)
    options_path = srcdir / '..' / 'compiler' / 'parser' / 'src' / 'options.rs'
    return srcdir, options_path.read_text()


def validate_flags(app, config):
    """Check every allow_* field in CompilerOptions has doc entries."""
    srcdir, options_text = _options_text(app)

    # Extract the --allow-* CLI flag string literals declared in the
    # define_compiler_options! macro (e.g. "--allow-c-style-comments").
    #
    # These literals are the FeatureDescriptor.cli_flag values and are the
    # source of truth. An earlier version of this guard matched
    # `pub (allow_\w+): bool` field declarations, but that regex silently
    # stopped matching — and so validated nothing — once the flags moved into
    # the macro (the fields are generated, not written literally). Matching the
    # CLI-flag literals is robust to that macro expansion.
    cli_flags = sorted(set(re.findall(r'"(--allow-[a-z0-9-]+)"', options_text)))
    if not cli_flags:
        print('ironplc_flags: no --allow-* flags found in options.rs; the '
              'source-of-truth format may have changed.')
        exit(1)

    # Verify each flag appears in both doc files
    doc_files = [
        'explanation/enabling-dialects-and-features.rst',
        'reference/compiler/ironplcc.rst',
    ]
    missing = []
    for doc_rel in doc_files:
        doc_path = srcdir / doc_rel
        doc_text = doc_path.read_text()
        for cli_flag in cli_flags:
            if cli_flag not in doc_text:
                missing.append(f'{cli_flag} missing in {doc_rel}')

    if missing:
        for m in missing:
            print(m)
        exit(1)


def validate_dialects(app, config):
    """Check every Dialect::cli_name() appears in the dialect doc pages.

    The `Dialect` set is the source of truth in options.rs. Several doc pages
    re-list the dialects by hand; this guard ties the "Supported Dialects"
    explanation and the editor settings reference back to `cli_name()` so a new
    dialect cannot ship undocumented (mirrors the --allow-* flag guard above).
    """
    srcdir, options_text = _options_text(app)

    # Extract the dialect cli_name() string literals. Scope the search to the
    # `fn cli_name` body so we do not accidentally pick up display_name() or
    # description() literals. Within that body each arm looks like
    # `Dialect::Variant => "iec61131-3-ed2",`.
    cli_name_match = re.search(
        r'fn cli_name\(.*?\)\s*->\s*&\'static str\s*\{(.*?)\n    \}',
        options_text,
        re.DOTALL,
    )
    if not cli_name_match:
        print('ironplc_flags: could not locate Dialect::cli_name() in '
              'options.rs; the source-of-truth format may have changed.')
        exit(1)

    dialect_names = sorted(set(re.findall(
        r'Dialect::\w+\s*=>\s*"([a-z0-9-]+)"',
        cli_name_match.group(1),
    )))
    if not dialect_names:
        print('ironplc_flags: no dialect cli_names found in options.rs; the '
              'source-of-truth format may have changed.')
        exit(1)

    # Verify each dialect appears in both dialect doc pages.
    doc_files = [
        'explanation/enabling-dialects-and-features.rst',
        'reference/editor/settings.rst',
    ]
    missing = []
    for doc_rel in doc_files:
        doc_path = srcdir / doc_rel
        doc_text = doc_path.read_text()
        for dialect_name in dialect_names:
            if dialect_name not in doc_text:
                missing.append(f'dialect {dialect_name} missing in {doc_rel}')

    if missing:
        for m in missing:
            print(m)
        exit(1)


def setup(app):
    app.connect('config-inited', validate_flags)
    app.connect('config-inited', validate_dialects)
    return {
        'version': '0.1',
        'parallel_read_safe': True,
        'parallel_write_safe': True,
    }
