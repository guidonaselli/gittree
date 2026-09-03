#!/usr/bin/env node
// Fails the build on any literal color or raw spacing value in a component
// (spec: ui-shell-and-theming, "Design-token architecture"; design D7,
// risk R8). Applied from F-001 onward, not deferred to the theming
// milestone, so no literal ever needs a costly retrofit.
//
// Scope: every .css/.tsx/.ts file under src/, EXCEPT theme/tokens.css
// (the tokens themselves) and theme/*-theme.* (built-in/user theme sources,
// which are also token-only files, just not this one).

import { readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { globSync } from "node:fs";

const SRC = new URL("../src", import.meta.url).pathname;
const EXEMPT = [/theme\/tokens\.css$/, /theme\/[\w-]+-theme\.(css|ts)$/];

const HEX_COLOR = /#[0-9a-fA-F]{3,8}\b/g;
const RGB_COLOR = /\brgba?\(\s*\d/g;
const RAW_PX = /(?<![\w-])(?:margin|padding|gap|top|left|right|bottom|width|height|border-radius|font-size|border-width)\s*:\s*-?\d+(\.\d+)?px/g;

function findFiles(dir) {
  return globSync("**/*.{css,ts,tsx}", { cwd: dir }).map((f) => join(dir, f));
}

let violations = [];
for (const file of findFiles(SRC)) {
  const rel = relative(SRC, file);
  if (EXEMPT.some((re) => re.test(rel))) continue;
  const text = readFileSync(file, "utf8");
  const lines = text.split("\n");
  lines.forEach((line, i) => {
    // Ignore comments and imports of asset URLs.
    if (line.trim().startsWith("//") || line.trim().startsWith("*")) return;
    for (const re of [HEX_COLOR, RGB_COLOR, RAW_PX]) {
      re.lastIndex = 0;
      const m = re.exec(line);
      if (m) {
        violations.push(`${rel}:${i + 1}: literal value "${m[0]}" — use a design token instead`);
      }
    }
  });
}

if (violations.length > 0) {
  console.error(`Token lint failed: ${violations.length} literal value(s) found outside the token system.\n`);
  for (const v of violations) console.error("  " + v);
  process.exit(1);
}

console.log("Token lint passed: no literal colors or raw spacing values outside theme/tokens.css.");
