// Generate `libs/index.json` for the served compatibility libraries.
//
// The playground activates a compatibility library by fetching its plain-text
// `.st` files, served alongside the app (REQ-CL-playground-001). Static hosts
// do not offer directory listing, so the browser cannot discover a library's
// files on its own. This build step scans the copied `libs/` tree and writes an
// index mapping each library name to the paths of its `.st` files (relative to
// the app root), which `app.ts` fetches to load a library.
//
// Usage: `node scripts/gen-libs-index.mjs <served-root>` (e.g. `_build`).

import { readdirSync, statSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";

const root = process.argv[2];
if (!root) {
  console.error("usage: gen-libs-index.mjs <served-root>");
  process.exit(1);
}

const libsDir = join(root, "libs");

// Recursively collect the `.st` files under `dir` (a library's version
// subdirectories hold the declarations).
function stFiles(dir) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      out.push(...stFiles(full));
    } else if (entry.endsWith(".st")) {
      out.push(full);
    }
  }
  return out;
}

// name -> { manifest: "libs/<Name>/library.toml",
//           files: [ "libs/<Name>/<version>/<file>.st", ... ] },
// paths relative to the served app root so the browser can fetch them
// directly. The manifest rides along because its bindings tell the compiler
// how intrinsic-bound and declare-only POUs compile.
const index = {};
for (const name of readdirSync(libsDir)) {
  const full = join(libsDir, name);
  if (!statSync(full).isDirectory()) {
    continue;
  }
  index[name] = {
    manifest: relative(root, join(full, "library.toml")),
    files: stFiles(full)
      .map((path) => relative(root, path))
      .sort(),
  };
}

writeFileSync(join(libsDir, "index.json"), `${JSON.stringify(index, null, 2)}\n`);
