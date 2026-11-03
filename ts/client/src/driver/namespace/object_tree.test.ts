// @pxl/client/driver/namespace/object_tree.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";
import * as fs from "node:fs";

import {
  fold,
  lay,
  shard_of,
  top_of,
  type Leaf,
  type TopRow,
} from "./object_tree.ts";

/** The record the kernel's store is held to too (pxl-kernel
 * domain/layer/object/repository.test.rs `top_golden`). */
const GOLDEN = new URL(
  "../../../../../rust/spec/golden/object/top.tsv",
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

test("top_of - each case's writes fold to the store's rows", () => {
  const g = records();
  let cases = 0;
  for (; g.has(`case.${cases}.write.0`); cases++) {
    const writes: Leaf[] = [];
    for (let j = 0; g.has(`case.${cases}.write.${j}`); j++) {
      const [key, id] = g.get(`case.${cases}.write.${j}`)!.split(" ");
      writes.push({ key: key!, id: Number(id), text: null });
    }
    const want: TopRow[] = [];
    for (let j = 0; g.has(`case.${cases}.top.${j}`); j++) {
      const [shard, z, f] = g.get(`case.${cases}.top.${j}`)!.split(" ");
      want.push({ shard: shard!, z: Number(z), fold: Number(f) });
    }
    const shards = new Map<string, Map<string, Leaf>>();
    for (const w of writes) {
      const shard = shard_of(w.key);
      if (!shards.has(shard)) shards.set(shard, new Map());
      lay(shards.get(shard)!, w);
    }
    const got = [...shards.keys()]
      .sort((a, b) => (a < b ? -1 : a > b ? 1 : 0))
      .flatMap((s) => top_of(s, shards.get(s)!.values()));
    assert.deepStrictEqual(got, want, `case ${cases}`);
  }
  assert.ok(cases > 0);
});

test("lay - a write splits the leaf over it and takes what is under it", () => {
  // A leaf at the first zoom past shard 0, then its first child of the next
  // zoom written: seven children of its value and the one written.
  const leaves = new Map<string, Leaf>();
  const coarse = "000000000";
  lay(leaves, { key: coarse, id: 7, text: null });
  const fine = "0000000000";
  lay(leaves, { key: fine, id: 8, text: null });
  assert.deepStrictEqual(leaves.size, 8);
  assert.deepStrictEqual(leaves.get(fine)?.id, 8);
  assert.ok(![...leaves.values()].some((l) => l.key === coarse));
  lay(leaves, { key: coarse, id: 0, text: null });
  assert.deepStrictEqual(leaves.size, 0);
});

test("fold - eight whole children of one kind fold to it whole", () => {
  const whole = (3 | (1 << 31)) >>> 0;
  assert.deepStrictEqual(fold(Array(8).fill(whole), 8), whole);
  assert.deepStrictEqual(fold(Array(8).fill(0), 9), 0);
});
