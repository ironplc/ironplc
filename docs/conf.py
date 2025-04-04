# Configuration file for the Sphinx documentation builder.

import codecs
from sys import path
from os.path import abspath

# -- Project information -----------------------------------------------------

project = 'IronPLC'
copyright = '2023, Garret Fick'
author = 'Garret Fick'

# -- General configuration ---------------------------------------------------

# Be strict about any broken references
nitpicky = True

# Add any Sphinx extension module names here, as strings. They can be
# extensions coming with Sphinx (named 'sphinx.ext.*') or your custom
# ones.
extensions = [
    "sphinx_design"
]

# Add any paths that contain templates here, relative to this directory.
templates_path = ['_templates']

# List of patterns, relative to source directory, that match files and
# directories to ignore when looking for source files.
# This pattern also affects html_static_path and html_extra_path.
exclude_patterns = ['_build', 'Thumbs.db', '.DS_Store']


# -- Options for HTML output -------------------------------------------------

# The theme to use for HTML and HTML Help pages.  See the documentation for
# a list of builtin themes.
#
html_theme = 'furo'

# Add any paths that contain custom static files (such as style sheets) here,
# relative to this directory. They are copied after the builtin static files,
# so a file named "default.css" will overwrite the builtin "default.css".
html_static_path = ['_static']
html_css_files = ["overrides.css"]

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

rst_prolog = """.. attention::
    These docs are a bit ambitious. The steps described are accurate but IronPLC cannot yet run programs.
"""

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
extensions += ["sphinx_inline_tabs", "sphinx.ext.extlinks", "sphinx.ext.autosectionlabel", "sphinx_copybutton", "ironplc_problemcode"]

autosectionlabel_prefix_document = True
