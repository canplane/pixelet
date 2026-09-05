// @pxl/web/modules/world/objects/store.test.ts

import { assertEquals, assertNotEquals } from "@std/assert";

import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { find_overlapping_object } from "./collision.ts";
import { DECOR_SIZE } from "./size.ts";
import { cell_index, cell_key, get_objects_in_cell, get_placed_object, place, place_decor, remove } from "./store.ts";

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
