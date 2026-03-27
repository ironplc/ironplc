'''
Validates that all --allow-* compiler flags from CompilerOptions are documented.

This extension reads the source of truth (compiler/parser/src/options.rs),
extracts all allow_* fields, and verifies each one appears in both the
enabling-features explanation page and the ironplcc CLI reference page.

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

    # Extract "allow_*" field names
    fields = re.findall(r'pub (allow_\w+): bool', options_text)
    # Edition flag is set by --dialect, not --allow-*
    fields = [f for f in fields if f != 'allow_iec_61131_3_2013']

    # Convert to CLI form: allow_foo_bar -> --allow-foo-bar
    cli_flags = {f: '--' + f.replace('_', '-') for f in fields}

    # Verify each flag appears in both doc files
    doc_files = [
        'explanation/enabling-features.rst',
        'reference/compiler/ironplcc.rst',
    ]
    missing = []
    for doc_rel in doc_files:
        doc_path = srcdir / doc_rel
        doc_text = doc_path.read_text()
        for field, cli_flag in cli_flags.items():
            if cli_flag not in doc_text:
                missing.append(f'{cli_flag} (from CompilerOptions.{field}) missing in {doc_rel}')

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
