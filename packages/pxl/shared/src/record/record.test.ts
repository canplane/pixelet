// @pxl/shared/record/record.test.ts

import { assert, assertEquals, assertThrows } from "@std/assert";

import { fnv1a64_str } from "@xpute/core/kit/hash.ts";
import type { U32Array, U8Array } from "@xpute/core/kit/array.ts";

import { encode_table, Table } from "./table.ts";
import { encode_placements, type Placement, PlacementTable } from "./placement.ts";
import { encode_palette, type PaletteEntry, PaletteTable, USER_ID_BASE } from "./palette.ts";
import { CHANNEL_ID_BASE, shipped_palette } from "./shipped.ts";
import { SHIPPED_SHEET, shipped_texture_key, shipped_texture_tile } from "./texture.ts";
import { cube_colors, cube_types, decor_types } from "../spec/cell/mod.ts";

Deno.test("table - a record names its kind, its columns and its count, and a reader finds columns by key", () => {
  const pkt = encode_table("thing", 3, 2, [
    { key: "a", value: new Uint32Array([1, 2]), required: true },
    { key: "s", value: ["x", "y"], required: false },
  ]);
  const t = new Table(pkt, ["a", "s"]);
  assertEquals(t.kind, "thing");
  assertEquals(t.version, 3);
  assertEquals(t.count, 2);
  assertEquals([...t.lanes<U32Array>("a")], [1, 2]);
  assertEquals(t.strings("s")!.at(1).get<string>(), "y");
  assertEquals(t.has("nope"), false);
  assertEquals(t.lanes_opt("nope"), null);
});

Deno.test("table - a column a reader does not know is skipped when ancillary and refused when required", () => {
  const pkt = encode_table("thing", 1, 1, [
    { key: "a", value: new Uint8Array([7]), required: true },
    { key: "later", value: new Float32Array([0.5]), required: false },
  ]);
  const t = new Table(pkt, ["a"]);
  assertEquals(t.lanes<U8Array>("a")[0], 7);

  const strict = encode_table("thing", 2, 1, [
    { key: "a", value: new Uint8Array([7]), required: true },
    { key: "must", value: new Float32Array([0.5]), required: true },
  ]);
  assertThrows(() => new Table(strict, ["a"]), Error, "required column must");
});

Deno.test("table - every column must have the record's row count", () => {
  assertThrows(() => encode_table("thing", 1, 2, [{ key: "a", value: new Uint8Array([1]) }]), Error, "has 1 rows, not 2");
});

Deno.test("placement - rows round-trip, ancillary columns only when some row has them", () => {
  const rows: Placement[] = [
    { id: fnv1a64_str("tree@1"), x: 12.5, y: 3.25, z: 40, size: 1, heading: Math.PI / 2, value: 7 },
    { id: 42n, x: 0, y: 1024, z: -2, size: 2, heading: 0, value: 3, scale: 1.25, tags: { name: "가" } },
  ];
  const t = new PlacementTable(encode_placements(rows));
  assertEquals(t.count, 2);
  assertEquals(t.id[0], fnv1a64_str("tree@1"));
  assertEquals(t.x[0], 12.5);
  assertEquals(t.size[1], 2);
  assert(Math.abs(t.heading[0] - Math.PI / 2) < 1e-6);
  assertEquals(t.value[1], 3);
  assertEquals(t.scale_of(0), 1);
  assertEquals(t.scale_of(1), 1.25);
  assertEquals(t.rotation, null);
  assertEquals(t.tags_of(0), undefined);
  assertEquals(t.tags_of(1), { name: "가" });
  assertEquals(t.row(1).tags, { name: "가" });

  const bare = new PlacementTable(encode_placements([rows[0]]));
  assertEquals(bare.scale, null);
  assertEquals(bare.tags_of(0), undefined);
});

Deno.test("placement - a full rotation is four per row", () => {
  const t = new PlacementTable(encode_placements([
    { id: 1n, x: 0, y: 0, z: 0, size: 1, heading: 0, value: 1, rotation: [0, 0, 0.7071, 0.7071] },
  ]));
  assert(Math.abs(t.rotation![2] - 0.7071) < 1e-6);
  assertEquals(t.row(0).rotation!.length, 4);
});

Deno.test("palette - rows round-trip and are found by id", () => {
  const rows: PaletteEntry[] = [
    { id: 1, kind: "cube", name: "white", model_hash: 0n, model_key: "", extent: [1, 1, 1], size: 1, license: "", author: "" },
    {
      id: USER_ID_BASE + 5,
      kind: "decor",
      name: "pine",
      model_hash: fnv1a64_str("sha256:abc"),
      model_key: "asset/abc",
      extent: [1.18, 1.33, 2.3],
      size: 1,
      license: "CC0",
      author: "Kenney",
    },
  ];
  const p = new PaletteTable(encode_palette(rows));
  assertEquals(p.count, 2);
  assertEquals(p.row_of(USER_ID_BASE + 5), 1);
  assertEquals(p.row_of(99), -1);
  assertEquals(p.kind_of(1), "decor");
  assertEquals(p.name_of(1), "pine");
  assertEquals(p.model_key_of(1), "asset/abc");
  assert(Math.abs(p.extent_max(1) - 2.3) < 1e-6);
  assertEquals(p.license_of(1), "CC0");
  assertEquals(p.entry(0).kind, "cube");
});

Deno.test("shipped palette - ids are unique, placeable values keep their numbers, every model row has an extent", () => {
  const p = shipped_palette();
  const seen = new Set<number>();
  for (let i = 0; i < p.count; i++) {
    assert(!seen.has(p.id[i]), `duplicate palette id ${p.id[i]}`);
    seen.add(p.id[i]);
  }
  assertEquals(p.name_of(p.row_of(cube_types.block.red)), "block.red");
  assertEquals(p.color_of(p.row_of(cube_types.block.red)), cube_colors.block[cube_types.block.red]);
  assertEquals(p.name_of(p.row_of(decor_types.tree)), "tree");
  assertEquals(p.kind_of(p.row_of(CHANNEL_ID_BASE.land + cube_types.land.sand)), "ground");
  const trees = p.rows_of_group("tree");
  assertEquals(trees.length, 1 + 9);
  for (const row of trees) assert(p.extent_max(row) > 2);
  assertEquals(p.rows_of_group("car").length, 5);
  // Every model row names its bytes and its skin by content, and the key
  // is what the client fetches.
  for (const row of [...trees, ...p.rows_of_group("person"), ...p.rows_of_group("pet")]) {
    if (p.model_key_of(row) === "") continue;
    assert(/^asset\/[0-9a-f]{64}$/.test(p.model_key_of(row)), p.model_key_of(row));
    assert(/^asset\/[0-9a-f]{64}$/.test(p.skin_key_of(row)), p.skin_key_of(row));
    assert(p.model_hash[row] !== 0n);
  }
  assertEquals(p.rows_of_group("person").length, 12);
  assertEquals(p.rows_of_group("pet").length, 24);
  assertEquals(p.kind_of(p.rows_of_group("pet")[0]), "actor");
});

Deno.test("shipped textures - a land top and side, a block top, and no block side", () => {
  assert(shipped_texture_key("land", "BARE", "top")?.startsWith("asset/"));
  assert(shipped_texture_key("land", "BARE", "side")?.startsWith("asset/"));
  assert(shipped_texture_key("block", "white", "top")?.startsWith("asset/"));
  assertEquals(shipped_texture_key("block", "white", "side"), null);
  assertEquals(shipped_texture_key("water", "RIVER", "side"), null);
  // The sheet holds every tile the keys name, each at a cell-aligned origin.
  assert(SHIPPED_SHEET.key.startsWith("asset/"));
  const tile = shipped_texture_tile("land", "BARE", "top")!;
  assertEquals(tile.x % SHIPPED_SHEET.cell, 0);
  assertEquals(tile.y % SHIPPED_SHEET.cell, 0);
  assertEquals(shipped_texture_tile("block", "white", "side"), null);
});
