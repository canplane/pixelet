// @pxl/web/modules/world/objects/store.test.ts

import { assertEquals, assertNotEquals } from "@std/assert";

import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { find_overlapping_object } from "./collision.ts";
import { DECOR_SIZE } from "./size.ts";
import { L2_IDX_IN_L3, L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";

import { cell_index, cell_is_occupied, cell_key, each_column_entry, get_objects_in_cell, get_placed_object, object_column_of, object_store_stats, place, place_decor, remove } from "./store.ts";

// Distinct, far-apart corners per test so leftover module-level state from
// one test never collides with another (this file has no localStorage under
// `deno test` — see store.ts's own `typeof localStorage === "undefined"`
// guard — so nothing persists between test files, but tests within this one
// file still share the same module-level chunk storage).
let next_cell_x = 0n;
function fresh_corner(): { i: ReturnType<typeof ivec.gather>; f: 0n } {
  const i = ivec.gather(next_cell_x, 0n, 0n);
  next_cell_x += 1n;
  return { i, f: 0n };
}

Deno.test("objects/store place_decor - stores a real decor entry, not a color", () => {
  const { i, f } = fresh_corner();
  place_decor(i, f, DECOR_SIZE, "tree");

  const [ix, iy, iz] = cell_index({ i, f });
  const entries = get_objects_in_cell(cell_key(ix, iy, iz));
  assertEquals(entries.length, 1);
  assertEquals(entries[0].kind, "decor");
  assertEquals(entries[0].kind === "decor" && entries[0].decor_id, "tree");
});

Deno.test("objects/store place_decor - is retrievable by its own leaf id, same path a solid cube uses", () => {
  const { i, f } = fresh_corner();
  place_decor(i, f, DECOR_SIZE, "tree");

  const [ix, iy, iz] = cell_index({ i, f });
  const [entry] = get_objects_in_cell(cell_key(ix, iy, iz));
  const fetched = get_placed_object(entry.id);
  assertNotEquals(fetched, null);
  assertEquals(fetched?.kind, "decor");
});

Deno.test("objects/store place_decor - blocks a second placement at the exact same corner (collision.ts treats decor as occupying its slot)", () => {
  const { i, f } = fresh_corner();
  place_decor(i, f, DECOR_SIZE, "tree");

  const blocked = find_overlapping_object({ i, f }, DECOR_SIZE, 0n);
  assertNotEquals(blocked, null);
  assertEquals(blocked?.kind, "decor");
});

Deno.test("objects/store place_decor - a solid cube still blocks placement the same way a decor entry does (both are OccupiedEntry)", () => {
  const { i, f } = fresh_corner();
  place(i, f, DECOR_SIZE, "red");

  const blocked = find_overlapping_object({ i, f }, DECOR_SIZE, 0n);
  assertNotEquals(blocked, null);
  assertEquals(blocked?.kind, "solid");
});

Deno.test("objects/store remove - erases a decor entry the same way it erases a solid cube", () => {
  const { i, f } = fresh_corner();
  place_decor(i, f, DECOR_SIZE, "tree");

  const [ix, iy, iz] = cell_index({ i, f });
  const key = cell_key(ix, iy, iz);
  const [entry] = get_objects_in_cell(key);

  remove(entry.id);
  assertEquals(get_objects_in_cell(key).length, 0);
  assertEquals(get_placed_object(entry.id), null);
});

Deno.test("objects/store column key - an object above one L2 span still belongs to its XY column", () => {
  // The renderer asks for a column by an XY page index and a z = 0 page
  // offset. A cell key that let its own Z through put everything above
  // absolute z = 64u under a key nothing ever queried: stored, hit-tested,
  // never drawn. Placement looked like it silently refused past a certain
  // height.
  const x = 4096n * 3n, y = 4096n * 5n;
  const ground = { i: ivec.gather(x, y, 10n), f: 0n as const };
  const high = { i: ivec.gather(x, y, 200n), f: 0n as const };
  place(ground.i, ground.f, DECOR_SIZE, "red");
  place(high.i, high.f, DECOR_SIZE, "blue");

  const column = TO_XY(ground.i);
  const key = object_column_of(L3_IDX(column), L2_IDX_IN_L3(column));
  const zs: bigint[] = [];
  each_column_entry(key, (entry) => zs.push(ivec.Z(entry.corner().i)));
  assertEquals(zs.sort((a, b) => Number(a - b)), [10n, 200n]);

  // And the renderer's own way of spelling the same key — a page offset it
  // builds with z = 0 — resolves to the same column.
  assertEquals(object_column_of(L3_IDX(column), ivec.gather(0n, 0n, 0n) | L2_IDX_IN_L3(column)), key);
});

Deno.test("objects/store band tree - a 2×2×2 block of one colour reads back as eight cells", () => {
  // A band tree merges a uniform block of whole cells into one larger leaf
  // when it compacts; everything downstream assumes an entry is one cell.
  const x0 = 4096n * 7n;
  const corners: { i: ReturnType<typeof ivec.gather>; f: 0n }[] = [];
  for (let dz = 0n; dz < 2n; dz++) for (let dy = 0n; dy < 2n; dy++) for (let dx = 0n; dx < 2n; dx++) corners.push({ i: ivec.gather(x0 + dx, dy, dz), f: 0n });
  for (const c of corners) place(c.i, c.f, DECOR_SIZE, "red");

  for (const c of corners) {
    const found = get_objects_in_cell(cell_key(...cell_index(c)));
    assertEquals(found.length, 1);
    assertEquals(found[0].size, 1);
  }
  const [first] = get_objects_in_cell(cell_key(...cell_index(corners[0])));
  remove(first.id);
  assertEquals(get_objects_in_cell(cell_key(...cell_index(corners[0]))).length, 0);
  assertEquals(get_objects_in_cell(cell_key(...cell_index(corners[7]))).length, 1);
});

Deno.test("objects/store band tree - clearing a chunk's last object frees the chunk", () => {
  const c = { i: ivec.gather(4096n * 9n, 4096n * 2n, 3n), f: 0n as const };
  const before = object_store_stats().chunks;
  place(c.i, c.f, DECOR_SIZE, "blue");
  assertEquals(object_store_stats().chunks, before + 1);
  const [entry] = get_objects_in_cell(cell_key(...cell_index(c)));
  remove(entry.id);
  assertEquals(object_store_stats().chunks, before);
});

Deno.test("objects/store entries - a column walk's chunk-local corner agrees with the absolute one", () => {
  const x0 = 4096n * 11n + 70n; // not on a chunk boundary
  const c = { i: ivec.gather(x0, 4096n * 4n + 5n, 130n), f: 0n as const };
  place(c.i, c.f, DECOR_SIZE, "red");
  const column = TO_XY(c.i);
  const base = object_column_of(L3_IDX(column), L2_IDX_IN_L3(column));
  let seen = 0;
  each_column_entry(base, (entry) => {
    seen++;
    const { i } = entry.corner();
    assertEquals(ivec.Z(i), 130n);
    assertEquals(entry.lx, Number(x0 % 64n));
    assertEquals(entry.ly, 5);
    assertEquals(entry.lz, 130);
  });
  assertEquals(seen, 1);
});

Deno.test("objects/store cells - occupancy follows placement and removal, and air does not occupy", () => {
  const c = { i: ivec.gather(4096n * 13n, 0n, 2n), f: 0n as const };
  const [ix, iy, iz] = cell_index(c);
  assertEquals(cell_is_occupied(ix, iy, iz), false);
  place(c.i, c.f, DECOR_SIZE, "red");
  assertEquals(cell_is_occupied(ix, iy, iz), true);
  const [entry] = get_objects_in_cell(cell_key(ix, iy, iz));
  remove(entry.id);
  assertEquals(cell_is_occupied(ix, iy, iz), false);
});
