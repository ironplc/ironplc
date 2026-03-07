"""
Sphinx extension that provides a ``.. playground::`` directive for embedding
the IronPLC Playground as an iframe on documentation pages.

Usage::

    .. playground::

       PROGRAM main
           VAR
               x : INT;
           END_VAR
           x := 42;
       END_PROGRAM

For snippets that need scaffolding (auto-wrapped in PROGRAM/VAR/END_PROGRAM)::

    .. playground::
       :vars: result : DINT;

       result := ABS(-42);

Options:
    :vars:    Variable declarations for scaffold mode (semicolon-separated).
    :height:  Iframe height (default "400px").
"""

from base64 import b64encode
from textwrap import dedent

from docutils import nodes
from docutils.parsers.rst import Directive, directives

PLAYGROUND_URL = "https://playground.ironplc.com/"


class PlaygroundDirective(Directive):
    has_content = True
    option_spec = {
        "vars": directives.unchanged,
        "height": directives.unchanged,
    }

    def run(self):
        code = "\n".join(self.content)
        height = self.options.get("height", "400px")
        vars_decl = self.options.get("vars", "")

        params = ["embed=true"]

        # Determine if scaffolding is needed
        trimmed = code.lstrip()
        needs_scaffold = not trimmed.upper().startswith(
            ("PROGRAM ", "FUNCTION_BLOCK ", "FUNCTION ")
        )

        if needs_scaffold:
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

        node = nodes.raw("", raw_html, format="html")
        return [node]


def setup(app):
    app.add_directive("playground", PlaygroundDirective)
    return {"version": "0.1", "parallel_read_safe": True}
