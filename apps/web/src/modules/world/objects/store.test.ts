// @pxl/web/modules/world/objects/store.test.ts

import { assertEquals, assertNotEquals } from "@std/assert";

import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { find_overlapping_object } from "./collision.ts";
import { DECOR_SIZE } from "./size.ts";
import { L2_IDX_IN_L3, L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";

import { cell_index, cell_key, get_object_entries_in_l2_column, get_objects_in_cell, get_placed_object, l2_column_key_of, place, place_decor, remove } from "./store.ts";

// Distinct, far-apart corners per test so leftover module-level state from
// one test never collides with another (this file has no localStorage under
// `deno test` — see store.ts's own `typeof localStorage === "undefined"`
// guard — so nothing persists between test files, but tests within this one
// file still share the same module-level `cells` map).
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
  const key = l2_column_key_of(L3_IDX(column), L2_IDX_IN_L3(column));
  const entries = get_object_entries_in_l2_column(key);
  assertEquals(entries.length, 2);
  assertEquals(entries.map((e) => ivec.Z(e.i)).sort((a, b) => Number(a - b)), [10n, 200n]);

  // And the renderer's own way of spelling the same key — a page offset it
  // builds with z = 0 — resolves to the same column.
  assertEquals(l2_column_key_of(L3_IDX(column), ivec.gather(0n, 0n, 0n) | L2_IDX_IN_L3(column)), key);
});
