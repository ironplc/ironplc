import * as assert from 'assert';
import * as path from 'path';
import * as fs from 'fs';

suite('problemUrls', () => {
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
