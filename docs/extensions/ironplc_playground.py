"""
Sphinx extension that provides directives for embedding the IronPLC Playground
as an iframe on documentation pages.

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

Options (both directives):
    :height:  Iframe height (default "400px").

Options (playground-with-program only):
    :vars:    Variable declarations for scaffold mode (semicolon-separated).
"""

from base64 import b64encode

from docutils import nodes
from docutils.parsers.rst import Directive, directives

PLAYGROUND_URL = "https://playground.ironplc.com/"


def _build_iframe(code, height, scaffold=False, vars_decl=""):
    params = ["embed=true"]

    if scaffold:
        params.append("scaffold=true")

    params.append("code=" + b64encode(code.encode()).decode())

    if vars_decl:
        params.append("vars=" + b64encode(vars_decl.encode()).decode())

    src = PLAYGROUND_URL + "?" + "&".join(params)

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
    }

    def run(self):
        code = "\n".join(self.content)
        height = self.options.get("height", "400px")
        return [_build_iframe(code, height)]


class PlaygroundWithProgramDirective(Directive):
    has_content = True
    option_spec = {
        "vars": directives.unchanged,
        "height": directives.unchanged,
    }

    def run(self):
        code = "\n".join(self.content)
        height = self.options.get("height", "400px")
        vars_decl = self.options.get("vars", "")
        return [_build_iframe(code, height, scaffold=True, vars_decl=vars_decl)]


def setup(app):
    app.add_directive("playground", PlaygroundDirective)
    app.add_directive("playground-with-program", PlaygroundWithProgramDirective)
    return {"version": "0.2", "parallel_read_safe": True}
