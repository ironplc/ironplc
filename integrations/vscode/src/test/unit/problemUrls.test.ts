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
  });

  test('problemHelpUrl_when_version_has_special_chars_then_encoded', () => {
    const url = problemHelpUrl('E0001', '1.0.0 beta');
    assert.ok(url.includes('version=1.0.0%20beta'));
  });

  test('openProblemInBrowser_when_url_path_then_docs_directory_exists', () => {
    // From out/test/unit/ -> repo root is 5 levels up (out/test/unit -> out/test -> out -> vscode -> integrations -> root)
    const repoRoot = path.resolve(__dirname, '..', '..', '..', '..', '..');
    const docsDir = path.join(repoRoot, 'docs', 'reference', 'editor', 'problems');
    assert.ok(fs.existsSync(docsDir), `Documentation directory does not exist: ${docsDir}`);

    const files = fs.readdirSync(docsDir);
    const hasErrorFiles = files.some(f => f.startsWith('E') && f.endsWith('.rst'));
    assert.ok(hasErrorFiles, `No E*.rst files found in ${docsDir}`);
  });
});
