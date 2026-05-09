"""
Sphinx extension that provides directives for embedding the IronPLC Playground
as an iframe on documentation pages and for generating playground links.

``.. playground::`` embeds the code as-is (no scaffolding). The directive
renders a "Try it" tab (the embedded playground iframe, shown by default)
and a "Source" tab containing the raw source. Both tabs are emitted into
the DOM so search engines can index the source — iframes are cross-origin
and contribute no text to the host page::

    .. playground::

       PROGRAM main
           VAR
               x : INT;
           END_VAR
           x := 42;
       END_PROGRAM

``.. playground-with-program::`` wraps the code in PROGRAM/VAR/END_PROGRAM
and renders the same tab pair::

    .. playground-with-program::
       :vars: result : DINT;

       result := ABS(-42);

``.. playground-link::`` generates a hyperlink to open the code in the
playground (no iframe, no tabs)::

    .. playground-link::
       :text: Open in Playground

       PROGRAM main
           VAR
               x : INT;
           END_VAR
           x := 42;
       END_PROGRAM

Options (all directives):
    :dialect: Force the playground dialect (e.g. ``2013`` for IEC 61131-3:2013).
    :allows:  Comma-separated list of ``--allow-*`` feature flags to enable on
              top of the dialect (e.g. ``sizeof,c-style-comments``). The short
              name is the part after ``--allow-``.

Options (both embed directives):
    :height:  Iframe height (default auto-calculated from line count).

Options (playground-with-program only):
    :vars:    Variable declarations for scaffold mode (semicolon-separated).

Options (playground-link only):
    :text:    Link text (default "Try this in the IronPLC Playground").
"""

from base64 import b64encode
from urllib.parse import quote

from docutils import nodes
from docutils.parsers.rst import Directive, directives

try:
    from sphinx_inline_tabs._impl import TabContainer
    _HAS_INLINE_TABS = True
except ImportError:
    _HAS_INLINE_TABS = False

PLAYGROUND_URL = "https://playground.ironplc.com/"

# Maximum number of visible code lines before the editor scrolls.
_MAX_VISIBLE_LINES = 15
# Minimum / maximum number of variable rows the output panel sizes for.
# The CSS in the playground enforces a matching minimum so at least
# _MIN_VISIBLE_VARS rows are always visible without scrolling.
_MIN_VISIBLE_VARS = 3
_MAX_VISIBLE_VARS = 8
# Minimum iframe height in pixels.
_MIN_HEIGHT_PX = 300


def _auto_height(code_lines, scaffold=False, vars_decl=""):
    """Calculate an iframe height that shows the example and its variables.

    The iframe contains two stacked panes in embed mode: the editor (top)
    and the output panel that hosts the variables table (bottom). Each
    pane is sized independently so adding more variables grows the
    output pane without squeezing the editor.
    """
    var_count = 0
    if scaffold and vars_decl:
        var_count = len([v for v in vars_decl.split(";") if v.strip()])

    editor_lines = code_lines
    if scaffold:
        # PROGRAM main + END_PROGRAM
        editor_lines += 2
        if var_count:
            # VAR + each declaration + END_VAR
            editor_lines += 2 + var_count

    editor_visible = min(editor_lines, _MAX_VISIBLE_LINES)
    # Editor pane: toolbar ≈ 42 px, padding ≈ 24 px, border ≈ 1 px ≈ 70 px
    # of chrome, plus ~20 px per visible line (0.8rem × 1.5 line-height).
    editor_px = editor_visible * 20 + 70

    # Output pane sizes for at least _MIN_VISIBLE_VARS rows so the table
    # is usable even when the example declares fewer variables, and grows
    # up to _MAX_VISIBLE_VARS so very long var lists scroll instead of
    # ballooning the iframe.
    # Add 3 extra slots: 2 synthesized time variables (__SYSTEM_UP_TIME and
    # __SYSTEM_UP_LTIME) that the runtime always injects, plus 1 row of padding.
    output_visible = min(max(var_count + 3, _MIN_VISIBLE_VARS), _MAX_VISIBLE_VARS)
    # Output pane: tabs ≈ 33 px, table header ≈ 24 px, panel padding ≈
    # 16 px ≈ 73 px of chrome, plus ~30 px per row (the sparkline cell
    # is 18 px tall with padding, which dominates the row pitch).
    output_px = output_visible * 30 + 73

    height = editor_px + output_px

    return f"{max(_MIN_HEIGHT_PX, height)}px"


def _build_playground_url(
    code, scaffold=False, vars_decl="", embed=False, dialect="", allows=""
):
    """Build a playground URL with encoded parameters."""
    params = []

    if embed:
        params.append("embed=true")

    if scaffold:
        params.append("scaffold=true")

    if dialect:
        params.append("dialect=" + quote(dialect, safe=""))

    if allows:
        params.append("allows=" + quote(allows, safe=","))

    params.append("code=" + quote(b64encode(code.encode()).decode(), safe=""))

    if vars_decl:
        params.append("vars=" + quote(b64encode(vars_decl.encode()).decode(), safe=""))

    return PLAYGROUND_URL + "?" + "&".join(params)


def _build_iframe(code, height, scaffold=False, vars_decl="", dialect="", allows=""):
    src = _build_playground_url(
        code,
        scaffold=scaffold,
        vars_decl=vars_decl,
        embed=True,
        dialect=dialect,
        allows=allows,
    )

    raw_html = (
        f'<iframe src="{src}" '
        f'class="ironplc-playground" '
        f'loading="lazy" '
        f'title="IronPLC Playground" '
        f'style="width:100%;height:{height};border:1px solid #3e3e5e;border-radius:4px;">'
        f"</iframe>"
    )

    return nodes.raw("", raw_html, format="html")


def _wrap_in_tabs(iframe_node, code):
    """Wrap an iframe in a "Try it" / "Source" tab pair.

    Both tabs render into the DOM (sphinx-inline-tabs hides the inactive one
    with CSS), so the source code is crawlable text on the page even though
    the cross-origin iframe contributes nothing. This prevents thin-page
    near-duplicate clustering by search engines while keeping the embedded
    playground as the default UX.

    Falls back to just the iframe if sphinx-inline-tabs is unavailable.
    """
    if not _HAS_INLINE_TABS:
        return [iframe_node]

    try_label = nodes.label("", "", nodes.Text("Try it"))
    try_content = nodes.container("", is_div=True, classes=["tab-content"])
    try_content += iframe_node
    try_tab = TabContainer("", type="tab", new_set=True)
    try_tab += try_label
    try_tab += try_content

    src_label = nodes.label("", "", nodes.Text("Source"))
    src_content = nodes.container("", is_div=True, classes=["tab-content"])
    src_content += nodes.literal_block(code, code)
    src_tab = TabContainer("", type="tab", new_set=False)
    src_tab += src_label
    src_tab += src_content

    return [try_tab, src_tab]


class PlaygroundDirective(Directive):
    has_content = True
    option_spec = {
        "height": directives.unchanged,
        "dialect": directives.unchanged,
        "allows": directives.unchanged,
    }

    def run(self):
        code = "\n".join(self.content)
        code_lines = len(self.content)
        height = self.options.get("height") or _auto_height(code_lines)
        dialect = self.options.get("dialect", "")
        allows = self.options.get("allows", "")
        iframe = _build_iframe(code, height, dialect=dialect, allows=allows)
        return _wrap_in_tabs(iframe, code)


class PlaygroundWithProgramDirective(Directive):
    has_content = True
    option_spec = {
        "vars": directives.unchanged,
        "height": directives.unchanged,
        "dialect": directives.unchanged,
        "allows": directives.unchanged,
    }

    def run(self):
        code = "\n".join(self.content)
        code_lines = len(self.content)
        vars_decl = self.options.get("vars", "")
        dialect = self.options.get("dialect", "")
        allows = self.options.get("allows", "")
        height = self.options.get("height") or _auto_height(
            code_lines, scaffold=True, vars_decl=vars_decl
        )
        iframe = _build_iframe(
            code,
            height,
            scaffold=True,
            vars_decl=vars_decl,
            dialect=dialect,
            allows=allows,
        )
        return _wrap_in_tabs(iframe, code)


_DEFAULT_LINK_TEXT = "Try this in the IronPLC Playground"


class PlaygroundLinkDirective(Directive):
    """Generate a hyperlink that opens the code in the playground."""

    has_content = True
    option_spec = {
        "text": directives.unchanged,
        "dialect": directives.unchanged,
        "allows": directives.unchanged,
    }

    def run(self):
        code = "\n".join(self.content)
        link_text = self.options.get("text", _DEFAULT_LINK_TEXT)
        dialect = self.options.get("dialect", "")
        allows = self.options.get("allows", "")
        href = _build_playground_url(code, dialect=dialect, allows=allows)

        ref_node = nodes.reference("", link_text, refuri=href, internal=False)
        ref_node["classes"].append("playground-link")
        paragraph = nodes.paragraph("", "", ref_node)
        return [paragraph]


def setup(app):
    app.add_directive("playground", PlaygroundDirective)
    app.add_directive("playground-with-program", PlaygroundWithProgramDirective)
    app.add_directive("playground-link", PlaygroundLinkDirective)
    return {"version": "0.3", "parallel_read_safe": True}
