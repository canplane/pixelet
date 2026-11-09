// @pxl/client/service/sys/fs/source.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";
import * as fs from "node:fs";

import { BASE_LAYERS } from "@/service/origin/publication.spec.ts";
import { decl } from "./source.ts";

/** pxl-spec is tested against the same record (bake/source.test.rs
 * `is_version_golden`). */
const GOLDEN = new URL(
  "../../../../../../spec/golden/bake/version.tsv",
  import.meta.url,
);

function records(): Map<string, string> {
  const map = new Map<string, string>();
  for (const line of fs.readFileSync(GOLDEN, "utf8").split("\n")) {
    const at = line.indexOf("\t");
    if (at >= 0) map.set(line.slice(0, at), line.slice(at + 1));
  }
  return map;
}

/** A declaration that gives every kind `version` and is valid otherwise. */
function declaring(version: string): string {
  const each = <T>(v: T) => Object.fromEntries(BASE_LAYERS.map((k) => [k, v]));
  return JSON.stringify({
    min_zoom: 0,
    format: each(0),
    version: each(version),
    shard: {},
  });
}

test("decl - refused where a version is no segment of a URL", () => {
  const g = records();
  let cases = 0;
  for (; g.has(`case.${cases}.version`); cases++) {
    const version = g.get(`case.${cases}.version`)!;
    const want = g.get(`case.${cases}.is_version`) === "true";
    assert.equal(decl(declaring(version)) !== null, want, version);
  }
  assert.ok(cases > 0);
});
