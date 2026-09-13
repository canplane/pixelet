// @pxl/web/module/world/page_table.test.ts

import { assertEquals } from "@std/assert";

import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { PageTable } from "./page_table.ts";
import { world_changes, world_wakers, WorldTopic } from "./change.ts";

const page = (x: bigint, y: bigint, z: bigint = 0n) => ivec.gather(x, y, z);

Deno.test("page_table - a page is found by its key, with Z ignored, and comes back as set", () => {
  const table = new PageTable<string>();
  table.set(page(3n, 4n), "a");
  assertEquals(table.get(page(3n, 4n)), "a");
  assertEquals(table.get(page(3n, 4n, 9n)), "a");
  assertEquals(table.has(page(4n, 3n)), false);
  assertEquals(Array.from(table.keys()), [page(3n, 4n)]);
});

Deno.test("page_table - clearing moves the last page into the hole and keeps every other pair", () => {
  const table = new PageTable<string>();
  table.set(page(1n, 1n), "a");
  table.set(page(2n, 2n), "b");
  table.set(page(3n, 3n), "c");
  assertEquals(table.clear(page(1n, 1n)), "a");
  assertEquals(table.size, 2);
  assertEquals(table.get(page(2n, 2n)), "b");
  assertEquals(table.get(page(3n, 3n)), "c");
  assertEquals(table.get(page(1n, 1n)), undefined);
  assertEquals(table.clear(page(1n, 1n)), undefined);
  assertEquals(new Set(table.entries()), new Set([[page(2n, 2n), "b"], [page(3n, 3n), "c"]]));
});

Deno.test("page_table - a key past 2^63 survives the round trip unsigned", () => {
  const table = new PageTable<number>();
  const key = page(1n << 20n, 1n << 20n);
  table.set(key, 1);
  const big = (1n << 63n) | key;
  table.set(big, 2);
  assertEquals(table.get(big), 2);
  assertEquals(Array.from(table.keys()).includes(big), true);
});

Deno.test("page_table - the revision moves on every set and clear", () => {
  const table = new PageTable<number>();
  table.set(page(1n, 0n), 1);
  table.set(page(1n, 0n), 2);
  table.clear(page(1n, 0n));
  assertEquals(table.revision, 3);
});

Deno.test("page_table - a table with a world topic writes its pages to the change log and heard at the pump", () => {
  const table = new PageTable<number>(WorldTopic.TERRAIN_PAGE);
  world_wakers.pump();
  const heard: (bigint | null)[] = [];
  const off = world_wakers.subscribe_topic(WorldTopic.TERRAIN_PAGE, (key) => heard.push(key === null ? null : BigInt.asUintN(64, key)));
  const before = table.revision;
  table.set(page(5n, 6n), 1);
  table.set(page(5n, 6n, 3n), 2);
  table.set(page(7n, 8n), 3);
  assertEquals(heard, [], "nothing runs inside the write");
  assertEquals(table.revision, before + 3);
  assertEquals(world_changes.revision_of(WorldTopic.TERRAIN_PAGE, page(5n, 6n)), before + 2);
  world_wakers.pump();
  assertEquals(heard, [page(5n, 6n), page(7n, 8n)], "each page once per pump, Z ignored");
  table.clear(page(7n, 8n));
  assertEquals(world_changes.revision_of(WorldTopic.TERRAIN_PAGE, page(7n, 8n)), 0, "a page that left is forgotten");
  off();
});
