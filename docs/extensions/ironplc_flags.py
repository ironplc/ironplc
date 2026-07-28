'''
Validates that all --allow-* compiler flags from CompilerOptions are documented.

This extension reads the source of truth (compiler/parser/src/options.rs),
extracts all allow_* fields, and verifies each one appears in both the
enabling-dialects-and-features explanation page and the ironplcc CLI reference page.

The build fails if any flag is missing from either doc file, ensuring
documentation stays in sync when new flags are added.
'''
import re
from pathlib import Path
from sys import exit


def validate_flags(app, config):
    """Check every allow_* field in CompilerOptions has doc entries."""
    srcdir = Path(app.srcdir)

    # Read source of truth
    options_path = srcdir / '..' / 'compiler' / 'parser' / 'src' / 'options.rs'
    options_text = options_path.read_text()

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


def setup(app):
    app.connect('config-inited', validate_flags)
    return {
        'version': '0.1',
        'parallel_read_safe': True,
        'parallel_write_safe': True,
    }
