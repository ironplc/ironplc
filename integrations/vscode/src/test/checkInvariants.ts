import * as fs from 'fs';
import * as path from 'path';

const packageJson = JSON.parse(fs.readFileSync(path.join(__dirname, '..', '..', 'package.json'), 'utf-8'));

// Exceptions: capabilities that are intentionally not tested, with justification.
// Each key is the capability ID; the value is the reason it is excluded.
const EXCEPTIONS = new Map<string, string>([
  ['plcopen-xml', 'Uses firstLine detection (no file extension); requires XML content matching which is not reliably testable via openTextDocument'],
]);

// Collect all test file contents
const testFiles = findTestFiles(path.join(__dirname));
const testContent = testFiles.map(f => fs.readFileSync(f, 'utf-8')).join('\n');

const failures: string[] = [];

// Check languages with extensions
for (const lang of packageJson.contributes.languages) {
  if (EXCEPTIONS.has(lang.id)) {
    continue;
  }
  if (lang.extensions && lang.extensions.length > 0) {
    if (!testContent.includes(lang.id)) {
      failures.push(`Language '${lang.id}' has no test reference`);
    }
  }
}

// Check that languages with extensions have a grammar assigned
const languagesWithGrammars = new Set<string>(
  packageJson.contributes.grammars.map((g: { language: string }) => g.language),
);
for (const lang of packageJson.contributes.languages) {
  if (EXCEPTIONS.has(lang.id)) {
    continue;
  }
  if (lang.extensions && lang.extensions.length > 0) {
    if (!languagesWithGrammars.has(lang.id)) {
      failures.push(`Language '${lang.id}' has no grammar assigned`);
    }
  }
}

// Check commands
for (const cmd of packageJson.contributes.commands) {
  if (EXCEPTIONS.has(cmd.command)) {
    continue;
  }
  if (!testContent.includes(cmd.command)) {
    failures.push(`Command '${cmd.command}' has no test reference`);
  }
}

// Check custom editors
for (const editor of packageJson.contributes.customEditors) {
  if (EXCEPTIONS.has(editor.viewType)) {
    continue;
  }
  if (!testContent.includes(editor.viewType)) {
    failures.push(`Custom editor '${editor.viewType}' has no test reference`);
  }
}

// Check task definitions
if (packageJson.contributes.taskDefinitions) {
  for (const taskDef of packageJson.contributes.taskDefinitions) {
    if (EXCEPTIONS.has(taskDef.type)) {
      continue;
    }
    if (!testContent.includes(taskDef.type)) {
      failures.push(`Task definition '${taskDef.type}' has no test reference`);
    }
  }
}

// Report exceptions for visibility
if (EXCEPTIONS.size > 0) {
  console.log('Exceptions (intentionally untested):');
  EXCEPTIONS.forEach((reason, id) => console.log(`  - ${id}: ${reason}`));
}

if (failures.length > 0) {
  console.error('Test coverage invariant failures:');
  failures.forEach(f => console.error(`  - ${f}`));
  process.exit(1);
}
else {
  console.log('All test coverage invariants satisfied.');
}

function findTestFiles(dir: string): string[] {
  const results: string[] = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      results.push(...findTestFiles(fullPath));
    }
    else if (entry.name.endsWith('.js')) {
      results.push(fullPath);
    }
  }
  return results;
}
