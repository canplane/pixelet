// @pxl/web/modules/world/anchor/registry.test.ts

import { assertEquals } from "@std/assert";

import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { AnchorRegistry } from "./registry.ts";

const idx = (x: bigint, y: bigint) => ivec.gather(x, y, 0n);

Deno.test("anchor registry - a group lives while anyone links it, and leaves the root when the last unlinks", () => {
  const registry = new AnchorRegistry();
  const a = registry.link(idx(1n, 2n));
  assertEquals(registry.link(idx(1n, 2n)), a);
  assertEquals(registry.size, 1);
  assertEquals(registry.root.children.length, 1);
  registry.unlink(idx(1n, 2n));
  assertEquals(registry.root.children.length, 1);
  registry.unlink(idx(1n, 2n));
  assertEquals(registry.size, 0);
  assertEquals(registry.root.children.length, 0);
  registry.unlink(idx(1n, 2n));
  assertEquals(registry.size, 0);
});

Deno.test("anchor registry - unlinking one anchor keeps every other anchor's group and count", () => {
  const registry = new AnchorRegistry();
  const a = registry.link(idx(1n, 1n));
  const b = registry.link(idx(2n, 2n));
  registry.link(idx(2n, 2n));
  const c = registry.link(idx(3n, 3n));
  registry.unlink(idx(1n, 1n));
  assertEquals(registry.size, 2);
  assertEquals(registry.link(idx(2n, 2n)), b);
  assertEquals(registry.link(idx(3n, 3n)), c);
  assertEquals(registry.root.children.includes(a), false);
  // b was linked three times by now and c twice: the counts moved with the groups.
  registry.unlink(idx(3n, 3n));
  registry.unlink(idx(3n, 3n));
  assertEquals(registry.root.children.includes(c), false);
  registry.unlink(idx(2n, 2n));
  registry.unlink(idx(2n, 2n));
  assertEquals(registry.root.children.includes(b), true);
  registry.unlink(idx(2n, 2n));
  assertEquals(registry.size, 0);
});

Deno.test("anchor registry - a shift of the origin moves every live group by the same delta", () => {
  const registry = new AnchorRegistry();
  const a = registry.link(idx(1n, 0n));
  const b = registry.link(idx(0n, 1n));
  const ax = a.position.x, ay = a.position.y, bx = b.position.x, by = b.position.y;
  registry.shift_map_origin(ivec.gather(10n, -20n, 0n));
  assertEquals([a.position.x - ax, a.position.y - ay], [-10, 20]);
  assertEquals([b.position.x - bx, b.position.y - by], [-10, 20]);
});
