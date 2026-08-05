import * as assert from 'assert';
import * as path from 'path';
import * as fs from 'fs';
import { problemHelpUrl } from '../../problemUrl';

suite('problemUrls', () => {
  test('problemHelpUrl_when_editor_code_then_tagged_for_extension', () => {
    const url = problemHelpUrl('E0001', '0.1.2');
    assert.ok(url.startsWith('https://www.ironplc.com/reference/editor/problems/E0001.html?'));
    assert.ok(url.includes('version=0.1.2'));
    assert.ok(url.includes('channel=extension'));
    assert.ok(!url.includes('utm_'));
  });

  test('problemHelpUrl_when_compiler_or_runtime_code_then_section_matches_prefix', () => {
    assert.ok(problemHelpUrl('P0001', '1.0.0').includes('/reference/compiler/problems/P0001.html'));
    assert.ok(problemHelpUrl('V6008', '1.0.0').includes('/reference/runtime/problems/V6008.html'));
    assert.ok(problemHelpUrl('H1001', '1.0.0').includes('/reference/playground/problems/H1001.html'));
  });

  test('problemHelpUrl_when_version_has_special_chars_then_encoded', () => {
    const url = problemHelpUrl('E0001', '1.0.0 beta');
    assert.ok(url.includes('version=1.0.0%20beta'));
  });

  test('problemHelpUrl_when_unknown_prefix_then_unknown_section', () => {
    // A future code family must not be silently attributed to an existing
    // section; it falls back to "unknown" (a 404) until mapped.
    assert.ok(problemHelpUrl('D0001', '1.0.0').includes('/reference/unknown/problems/D0001.html'));
  });

  // From out/test/unit/ -> repo root is 5 levels up
  // (out/test/unit -> out/test -> out -> vscode -> integrations -> root).
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..', '..');

  test('openProblemInBrowser_when_url_path_then_docs_directory_exists', () => {
    const docsDir = path.join(repoRoot, 'docs', 'reference', 'editor', 'problems');
    assert.ok(fs.existsSync(docsDir), `Documentation directory does not exist: ${docsDir}`);

    const files = fs.readdirSync(docsDir);
    const hasErrorFiles = files.some(f => f.startsWith('E') && f.endsWith('.rst'));
    assert.ok(hasErrorFiles, `No E*.rst files found in ${docsDir}`);
  });

  // Guards against a new documented code family (a new
  // docs/reference/<section>/problems/ directory) slipping past sectionForCode
  // and shipping a wrong or 404 link. Walks the real docs tree and asserts every
  // documented code's prefix maps to the section directory that holds its page.
  // If this fails, add the new prefix to sectionForCode in problemUrl.ts.
  test('problemHelpUrl_when_every_documented_code_then_section_matches_directory', () => {
    const referenceDir = path.join(repoRoot, 'docs', 'reference');
    let checked = 0;

    for (const section of fs.readdirSync(referenceDir)) {
      const problemsDir = path.join(referenceDir, section, 'problems');
      if (!fs.existsSync(problemsDir)) {
        continue; // Not every reference section has a problems/ dir.
      }
      for (const name of fs.readdirSync(problemsDir)) {
        const code = name.endsWith('.rst') ? name.slice(0, -'.rst'.length) : '';
        // Problem-code files are <UPPER><digits> (e.g. P0001); skip index.rst
        // and other prose pages.
        if (!/^[A-Z][0-9]+$/.test(code)) {
          continue;
        }
        assert.ok(
          problemHelpUrl(code, '0').includes(`/reference/${section}/problems/${code}.html`),
          `problemHelpUrl(${code}) should point at /reference/${section}/problems/ ` +
          `(its page lives there); a new code family needs a matching case in sectionForCode`,
        );
        checked++;
      }
    }

    assert.ok(checked > 0, `found no documented problem codes under ${referenceDir}`);
  });
});
