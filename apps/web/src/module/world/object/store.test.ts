// @pxl/web/module/world/object/store.test.ts

import { assertEquals, assertNotEquals } from "@std/assert";
import { world_wakers } from "@/module/world/change.ts";

import { fvec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { find_overlapping_object } from "./collision.ts";
import { DECOR_SIZE } from "./size.ts";
import { L2_IDX_IN_L3, L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";

import {
  cell_index,
  cell_is_occupied,
  cell_key,
  decode_objects,
  each_column_entry,
  encode_objects,
  get_objects_in_cell,
  get_objects_near,
  get_placed_object,
  object_column_of,
  object_store_stats,
  place,
  place_decor,
  remove,
  subscribe_decor_column,
  subscribe_object_column,
} from "./store.ts";
import { L2_BASE } from "@pxl/shared/spec/space/indexer.ts";

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

Deno.test("object/store place_decor - stores a real decor entry, not a color", () => {
  const { i, f } = fresh_corner();
  place_decor(i, f, DECOR_SIZE, "tree");

  const [ix, iy, iz] = cell_index({ i, f });
  const entries = get_objects_in_cell(cell_key(ix, iy, iz));
  assertEquals(entries.length, 1);
  assertEquals(entries[0].kind, "decor");
  assertEquals(entries[0].kind === "decor" && entries[0].decor_id, "tree");
});

Deno.test("object/store place_decor - is retrievable by its own leaf id, same path a solid cube uses", () => {
  const { i, f } = fresh_corner();
  place_decor(i, f, DECOR_SIZE, "tree");

  const [ix, iy, iz] = cell_index({ i, f });
  const [entry] = get_objects_in_cell(cell_key(ix, iy, iz));
  const fetched = get_placed_object(entry.id);
  assertNotEquals(fetched, null);
  assertEquals(fetched?.kind, "decor");
});

Deno.test("object/store place_decor - blocks a second placement at the exact same corner (collision.ts treats decor as occupying its slot)", () => {
  const { i, f } = fresh_corner();
  place_decor(i, f, DECOR_SIZE, "tree");

  const blocked = find_overlapping_object({ i, f }, DECOR_SIZE, 0n);
  assertNotEquals(blocked, null);
  assertEquals(blocked?.kind, "decor");
});

Deno.test("object/store place_decor - a solid cube still blocks placement the same way a decor entry does (both are OccupiedEntry)", () => {
  const { i, f } = fresh_corner();
  place(i, f, DECOR_SIZE, "red");

  const blocked = find_overlapping_object({ i, f }, DECOR_SIZE, 0n);
  assertNotEquals(blocked, null);
  assertEquals(blocked?.kind, "solid");
});

Deno.test("object/store remove - erases a decor entry the same way it erases a solid cube", () => {
  const { i, f } = fresh_corner();
  place_decor(i, f, DECOR_SIZE, "tree");

  const [ix, iy, iz] = cell_index({ i, f });
  const key = cell_key(ix, iy, iz);
  const [entry] = get_objects_in_cell(key);

  remove(entry.id);
  assertEquals(get_objects_in_cell(key).length, 0);
  assertEquals(get_placed_object(entry.id), null);
});

Deno.test("object/store column key - an object above one L2 span still belongs to its XY column", () => {
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

Deno.test("object/store band tree - a 2×2×2 block of one colour reads back as eight cells", () => {
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

Deno.test("object/store band tree - clearing a chunk's last object frees the chunk", () => {
  const c = { i: ivec.gather(4096n * 9n, 4096n * 2n, 3n), f: 0n as const };
  const before = object_store_stats().chunks;
  place(c.i, c.f, DECOR_SIZE, "blue");
  assertEquals(object_store_stats().chunks, before + 1);
  const [entry] = get_objects_in_cell(cell_key(...cell_index(c)));
  remove(entry.id);
  assertEquals(object_store_stats().chunks, before);
});

Deno.test("object/store entries - a column walk's chunk-local corner agrees with the absolute one", () => {
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

Deno.test("object/store cells - occupancy follows placement and removal, and air does not occupy", () => {
  const c = { i: ivec.gather(4096n * 13n, 0n, 2n), f: 0n as const };
  const [ix, iy, iz] = cell_index(c);
  assertEquals(cell_is_occupied(ix, iy, iz), false);
  place(c.i, c.f, DECOR_SIZE, "red");
  assertEquals(cell_is_occupied(ix, iy, iz), true);
  const [entry] = get_objects_in_cell(cell_key(ix, iy, iz));
  remove(entry.id);
  assertEquals(cell_is_occupied(ix, iy, iz), false);
});

Deno.test("object/store listeners - a column's subscribers hear its own changes at the next pump, on the right channel, until they unsubscribe", () => {
  const { i, f } = fresh_corner();
  const other = fresh_corner();
  const column = L2_BASE(TO_XY(i));
  world_wakers.pump();
  let objects = 0, decor = 0, elsewhere = 0;
  const off_objects = subscribe_object_column(column, () => objects++);
  const off_decor = subscribe_decor_column(column, () => decor++);
  const off_other = subscribe_object_column(L2_BASE(TO_XY(ivec.gather(1_000_000n, 1_000_000n, 0n))), () => elsewhere++);

  place_decor(i, f, DECOR_SIZE, "tree");
  assertEquals([objects, decor, elsewhere], [0, 0, 0], "nothing runs inside the write");
  world_wakers.pump();
  assertEquals([objects, decor, elsewhere], [1, 1, 0]);
  place_decor(other.i, other.f, DECOR_SIZE, "tree");
  world_wakers.pump();
  const same_column = L2_BASE(TO_XY(other.i)) === column;
  assertEquals([objects, decor, elsewhere], [same_column ? 2 : 1, same_column ? 2 : 1, 0]);

  off_objects();
  const before = decor;
  place(i, f, DECOR_SIZE, "red");
  world_wakers.pump();
  assertEquals(objects, same_column ? 2 : 1);
  assertEquals(decor > before, true);
  off_decor();
  off_other();
});

Deno.test("object/store persistence - a save is Placement records per page, and loading it puts every object back where it was", () => {
  // Far from every other test's cells, on its own page.
  const a = { i: ivec.gather(5000n, 7000n, 12n), f: 0n };
  const b = { i: ivec.gather(5001n, 7000n, 12n), f: fvec.gather(fvec.from_real(0.25), 0n, 0n) };
  place(a.i, a.f, 1, "red");
  place(b.i, b.f, 0.25, "blue");
  place_decor(ivec.gather(5000n, 7001n, 12n), 0n, DECOR_SIZE, "tree");
  const shape = (o: { i: bigint; f: bigint; size: number; kind: string }) => [o.i, o.f, o.size, o.kind];
  const before = get_objects_near(a, 1).map(shape);
  assertEquals(before.length, 3);
  const blob = encode_objects();

  for (const o of get_objects_near(a, 1)) remove(o.id);
  assertEquals(get_objects_near(a, 1).length, 0);

  decode_objects(blob);
  assertEquals(get_objects_near(a, 1).map(shape), before);
  assertEquals(encode_objects(), blob);
});
