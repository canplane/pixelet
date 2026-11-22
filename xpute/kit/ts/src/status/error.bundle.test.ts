// @xpute/kit/status/error.bundle.test.ts

/** Holds an error's place where every frame is one file's, as a bundle's
 * are: the error type and what it takes, each stripped of its types and its
 * imports, are joined into one module with a maker and its caller written
 * after them, and the place is the line that constructs the error. */

import { readFileSync } from "node:fs";
import { stripTypeScriptTypes } from "node:module";
import { runInThisContext } from "node:vm";
import { test } from "vitest";
import assert from "node:assert/strict";

/** The module `name` beside this one as a bundle holds it: its types
 * stripped, each line kept where it was, and its imports and exports gone,
 * its names the one module's own. */
function bundled(name: string): string {
  const src = readFileSync(new URL(name, import.meta.url), "utf8");
  return stripTypeScriptTypes(src)
    .replace(/^import\b[^;]*;$/gm, "")
    .replace(/^export \{[^}]*\};$/gm, "")
    .replace(/^export /gm, "");
}

test("an error's place is the line that constructs it where the constructor and its caller are one file's", () => {
  const parts = ["errno.spec.ts", "msg.spec.ts", "msg.ts", "error.ts"].map(
    bundled,
  );
  const lib = `(function () {\n${parts.join("\n")}\n`;
  const made = lib.split("\n").length;
  const src = `${lib}const make = () => new XputeError(ERRNO_EIO, "no café");
function caller() {
  return make();
}
return caller();
})()`;
  const e = runInThisContext(src, { filename: "bundle.js" }) as {
    file: string;
    line: number;
    msg: string;
  };
  assert.deepEqual([e.file, e.line, e.msg], ["bundle.js", made, "no café"]);
});
