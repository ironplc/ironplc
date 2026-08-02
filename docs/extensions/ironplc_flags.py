'''
Validates that all --allow-* compiler flags and every Dialect are documented.

This extension reads the source of truth (compiler/parser/src/options.rs),
extracts all allow_* CLI flags and every Dialect::cli_name(), and verifies each
one appears in the relevant doc pages.

It also verifies that each dialect's **Enables:** list in the explanation page
matches exactly the flags that dialect turns on in options.rs. The per-dialect
lists are the canonical place to learn which flags a dialect enables (the
individual flag entries no longer repeat this), so they must not drift.

The build fails if any flag or dialect is missing from its doc file(s), or if a
dialect's documented flag list disagrees with the source of truth, ensuring
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


# Maps the `Dialect` enum variant idents used in options.rs to the CLI names
# (and **bold** section headers) used in the explanation page.
DIALECT_IDENT_TO_CLI = {
    'Iec61131_3Ed2': 'iec61131-3-ed2',
    'Iec61131_3Ed3': 'iec61131-3-ed3',
    'Rusty': 'rusty',
    'Codesys': 'codesys',
    'TwinCat': 'twincat',
}

EXPLANATION_DOC = 'explanation/enabling-dialects-and-features.rst'


def source_of_truth_dialect_flags(options_text):
    """Map each dialect CLI name to the set of --allow-* flags it enables.

    Parses the ``define_compiler_options!`` macro entries, each of which pairs
    a ``"--allow-*"`` CLI-flag literal with a ``[Dialect, ...]`` list of the
    dialects that enable it.
    """
    truth = {cli: set() for cli in DIALECT_IDENT_TO_CLI.values()}
    entry_re = re.compile(r'"(--allow-[a-z0-9-]+)"\s*,\s*\[([^\]]*)\]')
    for cli_flag, dialect_group in entry_re.findall(options_text):
        for ident in re.findall(r'[A-Za-z0-9_]+', dialect_group):
            cli = DIALECT_IDENT_TO_CLI.get(ident)
            if cli is not None:
                truth[cli].add(cli_flag)
    return truth


def documented_dialect_flags(doc_text, problems):
    """Map each dialect to the flags listed after its **Enables:** label.

    Each dialect appears as a ``**<cli-name>**`` bold header in the "Supported
    Dialects" section, followed by prose and then a ``**Enables:**`` paragraph
    that lists the flags. Only that paragraph (up to the next blank line) is
    read, so flag names mentioned in the surrounding prose are not counted.
    """
    documented = {}
    for cli in DIALECT_IDENT_TO_CLI.values():
        header_idx = doc_text.find(f'**{cli}**')
        if header_idx == -1:
            problems.append(f'dialect header **{cli}** missing in {EXPLANATION_DOC}')
            continue
        enables_idx = doc_text.find('**Enables:**', header_idx)
        if enables_idx == -1:
            problems.append(f'**Enables:** list missing for dialect {cli} in {EXPLANATION_DOC}')
            continue
        end_idx = doc_text.find('\n\n', enables_idx)
        if end_idx == -1:
            end_idx = len(doc_text)
        region = doc_text[enables_idx:end_idx]
        documented[cli] = set(re.findall(r'--allow-[a-z0-9-]+', region))
    return documented


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
        EXPLANATION_DOC,
        'reference/compiler/ironplcc.rst',
    ]
    missing = []
    for doc_rel in doc_files:
        doc_path = srcdir / doc_rel
        doc_text = doc_path.read_text()
        for cli_flag in cli_flags:
            if cli_flag not in doc_text:
                missing.append(f'{cli_flag} missing in {doc_rel}')

    # Verify each dialect's **Enables:** list matches the source of truth.
    truth = source_of_truth_dialect_flags(options_text)
    explanation_text = (srcdir / EXPLANATION_DOC).read_text()
    documented = documented_dialect_flags(explanation_text, missing)
    for cli, expected in truth.items():
        if cli not in documented:
            continue  # a structural problem was already reported above
        actual = documented[cli]
        for extra in sorted(actual - expected):
            missing.append(
                f'{cli}: {extra} is listed under **Enables:** but the source of '
                f'truth does not enable it for this dialect'
            )
        for absent in sorted(expected - actual):
            missing.append(
                f'{cli}: {absent} is enabled by this dialect in options.rs but '
                f'is missing from its **Enables:** list'
            )

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
