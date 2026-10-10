// @pxl/shared/spec/space/sparse_tree/tree.test.ts

import { assertEquals } from "@std/assert";

import { BuddyAllocator } from "@xpute/core/ds/buddy.ts";
import { buddy_alloc } from "@xpute/core/ds/typed_alloc.ts";
import { pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";

import * as sparse_tree from "./tree.ts";

const LAYOUT = { dim: 3, min: -3, max: 6 } as const;

function at(x: number, y: number, z: number) {
  return pvf3.gather({ i: 0n, f: 0n }, pvf3.from_real(x), pvf3.from_real(y), pvf3.from_real(z));
}

Deno.test("sparse_tree retype - widening the leaf lane carries every value across", () => {
  const tree = sparse_tree.alloc(LAYOUT, 64, Uint8Array);
  sparse_tree.write(tree, at(3, 4, 5), 200, 0);
  sparse_tree.write(tree, at(10.125, 1, 0), 7);
  sparse_tree.retype(tree, Uint16Array);
  assertEquals(sparse_tree.read(tree, at(3.5, 4.5, 5.5)), 200);
  assertEquals(sparse_tree.read(tree, at(10.125, 1, 0)), 7);
  sparse_tree.write(tree, at(20, 20, 20), 300, 0);
  assertEquals(sparse_tree.read(tree, at(20, 20, 20)), 300);
});

Deno.test("sparse_tree compact - a pooled tree hands its old lanes back", () => {
  const pool = new BuddyAllocator({ min_block_bytes: 1 << 12, region_bytes: 1 << 20, max_idle_regions: 1, poison: false, bypass: false });
  const tree = sparse_tree.alloc(LAYOUT, 64, Uint8Array, buddy_alloc(pool));
  for (let x = 0; x < 16; x++) sparse_tree.write(tree, at(x, 0, 0), 1, 0);
  sparse_tree.compact(tree);
  assertEquals(pool.stat.live_blocks, 2, "only the compacted heap and data remain borrowed");
  sparse_tree.free(tree);
  assertEquals(pool.stat.live_blocks, 0);
});
