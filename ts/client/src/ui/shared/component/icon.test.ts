// @pxl/client/ui/shared/component/icon.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = fileURLToPath(new URL(".", import.meta.url));
const SRC = join(HERE, "../../..");

/** The glyphs the table draws, its keys as icon.tsx writes them. */
function glyphs(): string[] {
  const text = readFileSync(join(HERE, "icon.tsx"), "utf8");
  const table = text.slice(
    text.indexOf("const CARBON = {"),
    text.indexOf("} as const satisfies"),
  );
  return [...table.matchAll(/^\s+"?([a-z0-9.]+)"?: \[/gm)].map((m) => m[1]!);
}

/** Every glyph the UI's code names: a string given as a `glyph` or an `icon`,
 * in the expression that gives it, and every string in a file that
 * names the glyphs' type, where tables of glyphs stand. The icon's own files
 * and the tests are left out. */
function quoted(): Set<string> {
  const out = new Set<string>();
  for (const file of readdirSync(SRC, { recursive: true, encoding: "utf8" })) {
    if (!/\.tsx?$/.test(file) || /\.test\.tsx?$/.test(file)) continue;
    if (/component\/icon(\.ios)?\.tsx$/.test(file)) continue;
    const text = readFileSync(join(SRC, file), "utf8");
    const spans = /\bGlyphName\b/.test(text)
      ? [text]
      : [
          ...text.matchAll(/\b(?:glyph|icon)\s*[=:]\s*(\{[^}]*\}|[^,\n]*)/g),
        ].map((m) => m[1]!);
    for (const span of spans) {
      for (const m of span.matchAll(/"([a-z0-9.]+)"/g)) out.add(m[1]!);
    }
  }
  return out;
}

test("CARBON - holds only the glyphs the UI names", () => {
  const used = quoted();
  assert.deepStrictEqual(
    glyphs().filter((g) => !used.has(g)),
    [],
  );
});
