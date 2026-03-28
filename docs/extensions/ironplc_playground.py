"""
Sphinx extension that provides directives for embedding the IronPLC Playground
as an iframe on documentation pages and for generating playground links.

``.. playground::`` embeds the code as-is (no scaffolding)::

    .. playground::

       PROGRAM main
           VAR
               x : INT;
           END_VAR
           x := 42;
       END_PROGRAM

``.. playground-with-program::`` wraps the code in PROGRAM/VAR/END_PROGRAM::

    .. playground-with-program::
       :vars: result : DINT;

       result := ABS(-42);

``.. playground-link::`` generates a hyperlink to open the code in the
playground (no iframe)::

    .. playground-link::
       :text: Open in Playground

       PROGRAM main
           VAR
               x : INT;
           END_VAR
           x := 42;
       END_PROGRAM

Options (both embed directives):
    :height:  Iframe height (default auto-calculated from line count).

Options (playground-with-program only):
    :vars:    Variable declarations for scaffold mode (semicolon-separated).

Options (playground-link only):
    :text:    Link text (default "Try this in the IronPLC Playground").
"""

from base64 import b64encode
from math import ceil
from urllib.parse import quote

from docutils import nodes
from docutils.parsers.rst import Directive, directives

PLAYGROUND_URL = "https://playground.ironplc.com/"

# Maximum number of visible lines before the editor scrolls.
_MAX_VISIBLE_LINES = 15
# Minimum iframe height in pixels.
_MIN_HEIGHT_PX = 300


def _auto_height(code_lines, scaffold=False, vars_decl=""):
    """Calculate an iframe height that shows the full example (up to a limit).

    Accounts for the scaffold wrapper lines that the playground adds when
    ``scaffold=true`` (PROGRAM, VAR/END_VAR, END_PROGRAM) so the editor is
    tall enough to display them without scrolling.
    """
    total_lines = code_lines

    if scaffold:
        # PROGRAM main + END_PROGRAM
        total_lines += 2
        if vars_decl:
            var_count = len([v for v in vars_decl.split(";") if v.strip()])
            # VAR + each declaration + END_VAR
            total_lines += 2 + var_count

    visible = min(total_lines, _MAX_VISIBLE_LINES)

    # The editor section gets ~70% of total height (see embed CSS).
    # Inside that: toolbar ≈ 42 px, padding ≈ 24 px, border ≈ 1 px → 67 px.
    # Use 70 px to leave a small safety margin for font-rendering variance.
    # Each line ≈ 20 px (0.8rem font × 1.5 line-height at 16px base).
    # Solve for total:  0.70 × total − 70 ≥ visible × 20
    #                   total ≥ (visible × 20 + 70) / 0.70
    height = ceil((visible * 20 + 70) / 0.70)

    return f"{max(_MIN_HEIGHT_PX, height)}px"


def _build_playground_url(code, scaffold=False, vars_decl="", embed=False, dialect=""):
    """Build a playground URL with encoded parameters."""
    params = []

    if embed:
        params.append("embed=true")

    if scaffold:
        params.append("scaffold=true")

    if dialect:
        params.append("dialect=" + quote(dialect, safe=""))

    params.append("code=" + quote(b64encode(code.encode()).decode(), safe=""))

    if vars_decl:
        params.append("vars=" + quote(b64encode(vars_decl.encode()).decode(), safe=""))

    return PLAYGROUND_URL + "?" + "&".join(params)


def _build_iframe(code, height, scaffold=False, vars_decl="", dialect=""):
    src = _build_playground_url(
        code, scaffold=scaffold, vars_decl=vars_decl, embed=True, dialect=dialect
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


class PlaygroundDirective(Directive):
    has_content = True
    option_spec = {
        "height": directives.unchanged,
        "dialect": directives.unchanged,
    }

    def run(self):
        code = "\n".join(self.content)
        code_lines = len(self.content)
        height = self.options.get("height") or _auto_height(code_lines)
        dialect = self.options.get("dialect", "")
        return [_build_iframe(code, height, dialect=dialect)]


class PlaygroundWithProgramDirective(Directive):
    has_content = True
    option_spec = {
        "vars": directives.unchanged,
        "height": directives.unchanged,
        "dialect": directives.unchanged,
    }

    def run(self):
        code = "\n".join(self.content)
        code_lines = len(self.content)
        vars_decl = self.options.get("vars", "")
        dialect = self.options.get("dialect", "")
        height = self.options.get("height") or _auto_height(
            code_lines, scaffold=True, vars_decl=vars_decl
        )
        return [_build_iframe(code, height, scaffold=True, vars_decl=vars_decl, dialect=dialect)]


_DEFAULT_LINK_TEXT = "Try this in the IronPLC Playground"


class PlaygroundLinkDirective(Directive):
    """Generate a hyperlink that opens the code in the playground."""

    has_content = True
    option_spec = {
        "text": directives.unchanged,
        "dialect": directives.unchanged,
    }

    def run(self):
        code = "\n".join(self.content)
        link_text = self.options.get("text", _DEFAULT_LINK_TEXT)
        dialect = self.options.get("dialect", "")
        href = _build_playground_url(code, dialect=dialect)

        ref_node = nodes.reference("", link_text, refuri=href, internal=False)
        ref_node["classes"].append("playground-link")
        paragraph = nodes.paragraph("", "", ref_node)
        return [paragraph]


def setup(app):
    app.add_directive("playground", PlaygroundDirective)
    app.add_directive("playground-with-program", PlaygroundWithProgramDirective)
    app.add_directive("playground-link", PlaygroundLinkDirective)
    return {"version": "0.3", "parallel_read_safe": True}
