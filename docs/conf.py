# Configuration file for the Sphinx documentation builder.

import codecs
from sys import path
from os.path import abspath

# -- Project information -----------------------------------------------------

project = 'IronPLC'
copyright = '2023-2026, Garret Fick'
author = 'Garret Fick'

# -- General configuration ---------------------------------------------------

# Be strict about any broken references
nitpicky = True

# Add any Sphinx extension module names here, as strings. They can be
# extensions coming with Sphinx (named 'sphinx.ext.*') or your custom
# ones.
extensions = [
    "sphinx_design",
    "sphinx_sitemap",
    "sphinxext.opengraph",
]

# Add any paths that contain templates here, relative to this directory.
templates_path = ['_templates']

# List of patterns, relative to source directory, that match files and
# directories to ignore when looking for source files.
# This pattern also affects html_static_path and html_extra_path.
exclude_patterns = ['_build', 'Thumbs.db', '.DS_Store', 'includes', '.venv']


# -- Options for HTML output -------------------------------------------------

# The theme to use for HTML and HTML Help pages.  See the documentation for
# a list of builtin themes.
#
html_theme = 'furo'
html_scaled_image_link = False
html_baseurl = "https://www.ironplc.com/"
html_title = "IronPLC - Open-Source IEC 61131-3 Toolchain"

# Add any paths that contain custom static files (such as style sheets) here,
# relative to this directory. They are copied after the builtin static files,
# so a file named "default.css" will overwrite the builtin "default.css".
html_static_path = ['_static']
html_extra_path = ['robots.txt']
html_css_files = ["overrides.css"]
html_js_files = ["version-check.js"]

html_theme_options = {
    "light_css_variables": {
        "admonition-font-size": "100%",
        "admonition-title-font-size": "100%",
        "color-brand-primary": "#C44536",
        "color-brand-content": "#00808b",
        "color-announcement-background": "#711818de",
        "color-announcement-text": "#fff"
    },
    "dark_css_variables": {
        "color-brand-primary": "#ed9d13",
        "color-brand-content": "#58d3ff",
        "color-announcement-background": "#711818de",
        "color-announcement-text": "#fff"
    },
    # Yes, I'm aware that this footer icon also includes the script for tracking. Either accept it or tell me a better way.
    "footer_icons": [
        {
            "name": "GitHub",
            "url": "https://github.com/ironplc/ironplc",
            "html": """
                <svg stroke="currentColor" fill="currentColor" stroke-width="0" viewBox="0 0 16 16">
                    <path fill-rule="evenodd" d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8z"></path>
                </svg>
                <script async data-id="101424946" src="https://static.getclicky.com/js"></script>
            """,
            "class": "",
        }
    ],
    "source_repository": "https://github.com/ironplc/ironplc/",
    "source_branch": "main",
    "source_directory": "docs/"
}

html_favicon = '_static/favicon.ico'

# -- Version configuration ---------------------------------------------------

# Gets the version number by reading from the VERSION file in this folder
with open("VERSION", "rb") as fp:
    encoded_text = fp.read()

    if encoded_text.startswith(codecs.BOM_UTF16_LE):
        encoded_text = encoded_text[len(codecs.BOM_UTF16_LE):]
        decoded_text = encoded_text.decode('utf-16le')
    else:
        decoded_text = encoded_text.decode('utf-8')
    
    version = str(decoded_text).strip()

# -- Extensions configuration ---------------------------------------------------

path.append(abspath("./extensions"))

extlinks = {'download_artifact': ('https://github.com/ironplc/ironplc/releases/download/v' + version + '/%s',
                      '%s')}
extensions += ["sphinx_inline_tabs", "sphinx.ext.extlinks", "sphinx.ext.autosectionlabel", "sphinx_copybutton", "ironplc_problemcode", "ironplc_playground", "ironplc_flags", "ironplc_redirects"]

autosectionlabel_prefix_document = True

# -- Redirects for restructured pages ---------------------------------------
# Maps previously-indexed URL paths to their current locations so external
# links (and Google's index) keep working after a restructure. Keys and
# values are paths relative to the site root. Add an entry whenever a page
# is moved or renamed.
_problems_index = "reference/compiler/problems/index.html"
ironplc_redirects = {
    # /compiler/* moved under /reference/compiler/*
    "compiler/index.html": "reference/compiler/index.html",
    "compiler/basicusage.html": "reference/compiler/overview.html",
    "compiler/source-formats/plcopen-xml.html": "reference/compiler/source-formats/plcopen-xml.html",
    "compiler/source-formats/text.html": "reference/compiler/source-formats/text.html",
    "compiler/source-formats/twincat.html": "reference/compiler/source-formats/twincat.html",

    # P0008 and P0009 still exist under the new path; the rest of the old
    # P00xx codes were renumbered into the P2xxx/P4xxx scheme with no 1:1
    # mapping, so they fall back to the problems index.
    "compiler/problems/P0008.html": "reference/compiler/problems/P0008.html",
    "compiler/problems/P0009.html": "reference/compiler/problems/P0009.html",
    "compiler/problems/P0012.html": _problems_index,
    "compiler/problems/P0013.html": _problems_index,
    "compiler/problems/P0014.html": _problems_index,
    "compiler/problems/P0015.html": _problems_index,
    "compiler/problems/P0017.html": _problems_index,
    "compiler/problems/P0019.html": _problems_index,
    "compiler/problems/P0020.html": _problems_index,
    "compiler/problems/P0021.html": _problems_index,
    "compiler/problems/P0023.html": _problems_index,
    "compiler/problems/P0025.html": _problems_index,
    "compiler/problems/P0027.html": _problems_index,
    "compiler/problems/P0029.html": _problems_index,
    "compiler/problems/P0031.html": _problems_index,
    "compiler/problems/P0032.html": _problems_index,
    "compiler/problems/P0033.html": _problems_index,
    "compiler/problems/P0034.html": _problems_index,
    "compiler/problems/P0035.html": _problems_index,
    "compiler/problems/P0036.html": _problems_index,
    "compiler/problems/P0037.html": _problems_index,
    "compiler/problems/P0040.html": _problems_index,
    "compiler/problems/P0041.html": _problems_index,
    "compiler/problems/P0042.html": _problems_index,
    "compiler/problems/P0043.html": _problems_index,
    "compiler/problems/P0044.html": _problems_index,
    "compiler/problems/P0046.html": _problems_index,
    "compiler/problems/P0048.html": _problems_index,

    # Beremiz and TwinCAT how-to guides moved into per-tool subdirectories.
    "how-to-guides/check-beremiz-projects.html": "how-to-guides/beremiz/check-beremiz-projects.html",
    "how-to-guides/check-twincat-projects.html": "how-to-guides/twincat/check-twincat-projects.html",

    # Data types split into elementary/ and derived/ subdirectories.
    "reference/language/data-types/dint.html": "reference/language/data-types/elementary/dint.html",
    "reference/language/data-types/int.html": "reference/language/data-types/elementary/int.html",
    "reference/language/data-types/lreal.html": "reference/language/data-types/elementary/lreal.html",
    "reference/language/data-types/ltime-of-day.html": "reference/language/data-types/elementary/ltime-of-day.html",
    "reference/language/data-types/lword.html": "reference/language/data-types/elementary/lword.html",
    "reference/language/data-types/ulint.html": "reference/language/data-types/elementary/ulint.html",
    "reference/language/data-types/word.html": "reference/language/data-types/elementary/word.html",
    "reference/language/data-types/reference-types.html": "reference/language/data-types/derived/reference-types.html",
    "reference/language/data-types/subrange-types.html": "reference/language/data-types/derived/subrange-types.html",

    # /vscode/* moved under /reference/editor/* (and troubleshooting joined how-to-guides).
    "vscode/overview.html": "reference/editor/overview.html",
    "vscode/settings.html": "reference/editor/settings.html",
    "vscode/troubleshooting.html": "how-to-guides/troubleshoot-editor.html",
}

# -- Open Graph configuration --------------------------------------------------

ogp_site_url = "https://www.ironplc.com/"
ogp_site_name = "IronPLC"
ogp_description_length = 200

# -- Sitemap configuration -----------------------------------------------------

sitemap_url_scheme = "{link}"
