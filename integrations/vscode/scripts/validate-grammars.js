// validate-grammars.js
//
// Validates the TextMate grammar JSON files in syntaxes/ against the
// JSON Schema in schemas/tmlanguage.json. Replaces the previous
// `ajv validate ...` CLI invocation from ajv-cli, which is unmaintained
// and pulled in a vulnerable version of fast-json-patch.

'use strict';

const fs = require('fs');
const path = require('path');
const Ajv = require('ajv-draft-04');
const { globSync } = require('glob');

const ROOT = path.resolve(__dirname, '..');
const SCHEMA_PATH = path.join(ROOT, 'schemas', 'tmlanguage.json');
const GRAMMAR_GLOB = 'syntaxes/*.tmLanguage.json';

function main() {
  const schema = JSON.parse(fs.readFileSync(SCHEMA_PATH, 'utf8'));
  const ajv = new Ajv({ allErrors: true, strict: false });
  const validate = ajv.compile(schema);

  const files = globSync(GRAMMAR_GLOB, { cwd: ROOT, absolute: true }).sort();
  if (files.length === 0) {
    console.error(`No grammar files matched ${GRAMMAR_GLOB} under ${ROOT}`);
    process.exit(1);
  }

  let hadFailure = false;
  for (const file of files) {
    const rel = path.relative(ROOT, file);
    const data = JSON.parse(fs.readFileSync(file, 'utf8'));
    if (validate(data)) {
      console.log(`${rel} valid`);
    } else {
      hadFailure = true;
      console.error(`${rel} INVALID`);
      console.error(ajv.errorsText(validate.errors, { separator: '\n  ', dataVar: rel }));
    }
  }

  process.exit(hadFailure ? 1 : 0);
}

main();
