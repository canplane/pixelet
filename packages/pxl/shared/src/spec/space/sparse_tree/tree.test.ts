// @pxl/shared/spec/space/sparse_tree/tree.test.ts

import { assertEquals } from "@std/assert";

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
