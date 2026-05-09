'''
Emits HTML redirect stubs at old URL paths so previously-indexed pages still
resolve after a content restructure.

Reads ``ironplc_redirects`` from conf.py: a dict mapping old build-root-relative
paths (the URLs Google has indexed) to new build-root-relative paths. Each old
path becomes a small static HTML file with ``<meta http-equiv="refresh">`` and
``<link rel="canonical">`` pointing at the new URL, plus ``noindex`` so Google
treats the stub itself as throwaway and consolidates link signals on the target.

The stubs are written after Sphinx finishes building, into the output directory
alongside the rest of the site.
'''
from pathlib import Path


_TEMPLATE = '''<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Redirecting&hellip;</title>
<link rel="canonical" href="{new_url}">
<meta http-equiv="refresh" content="0; url={new_url}">
<meta name="robots" content="noindex">
</head>
<body>
<p>This page has moved. <a href="{new_url}">Continue to the new location</a>.</p>
</body>
</html>
'''


def _emit_redirects(app, exception):
    if exception is not None:
        return
    redirects = app.config.ironplc_redirects or {}
    if not redirects:
        return
    base_url = (app.config.html_baseurl or '').rstrip('/')
    outdir = Path(app.outdir)
    for old_path, new_path in redirects.items():
        new_url = f"{base_url}/{new_path.lstrip('/')}"
        target = outdir / old_path.lstrip('/')
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(_TEMPLATE.format(new_url=new_url), encoding='utf-8')


def setup(app):
    app.add_config_value('ironplc_redirects', {}, 'env')
    app.connect('build-finished', _emit_redirects)
    return {'version': '1.0', 'parallel_read_safe': True}
