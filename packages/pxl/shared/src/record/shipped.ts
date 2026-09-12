// @pxl/shared/record/shipped.ts

/**
 * The palette this build ships: every cube type, every decor kind and its
 * models, and every character, as one record.
 *
 * Ids. A placeable value — a block colour, AIR, a decor kind — keeps the
 * number the object store already writes (spec/cell), which is under 256.
 * A terrain channel's types overlap each other by number, since each is
 * its own lane, so here each channel is offset into its own range. A
 * model is a row from MODEL_ID_BASE, one per glb, grouped by the scatter
 * kind that uses it; a kind's variant order is this file's row order. A
 * character is a row from ACTOR_ID_BASE, grouped "person" or "pet".
 *
 * What a model *is* — its bytes, its bounds — comes from ingested.ts, the
 * table apps/api/src/bake/asset/tools/ingest_asset.ts generates from the
 * files under asset/: the bucket key is the sha256 of the glb and the
 * extent is measured from it. This file only says which models there are,
 * in what order, under which kind, and with which colormap.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import { fnv1a64_str } from "@xpute/core/kit/hash.ts";

import { cube_colors, cube_types, decor_types } from "../spec/cell/mod.ts";
import { INGESTED } from "./ingested.ts";
import { encode_palette, type PaletteEntry, type PaletteKind, PaletteTable } from "./palette.ts";

/** Terrain channels' types, offset so a global id is unique. */
export const CHANNEL_ID_BASE = { land: 0x100, water: 0x200, structure: 0x300, transport: 0x400 } as const;
export const MODEL_ID_BASE: u32 = 0x1000;
export const ACTOR_ID_BASE: u32 = 0x2000;

const KENNEY = { license: "CC0-1.0", author: "Kenney" } as const;

/** A pack's models share one colormap. */
type Pack = { dir: string; skin: string };

const PACKS = {
  fantasy_town: { dir: "decor/kenney_fantasy_town", skin: "colormap.png" },
  graveyard: { dir: "decor/kenney_graveyard_kit", skin: "colormap.png" },
  car: { dir: "decor/kenney_car_kit", skin: "colormap.png" },
  train: { dir: "decor/kenney_train_kit", skin: "colormap.png" },
  watercraft: { dir: "decor/kenney_watercraft_pack", skin: "colormap.png" },
  mini_characters: { dir: "actor/kenney_mini_characters", skin: "colormap.png" },
  cube_pets: { dir: "actor/kenney_cube_pets", skin: "colormap.png" },
} as const satisfies Record<string, Pack>;

/** A model: its pack and its file, in the order that is the kind's variant
 * order. */
type Model = [pack: keyof typeof PACKS, file: string];

const MODELS: Record<string, readonly Model[]> = {
  tree: [
    ["fantasy_town", "tree.glb"],
    ["fantasy_town", "tree-high.glb"],
    ["fantasy_town", "tree-crooked.glb"],
    ["fantasy_town", "tree-high-round.glb"],
    ["fantasy_town", "tree-high-crooked.glb"],
    ["graveyard", "pine.glb"],
    ["graveyard", "pine-crooked.glb"],
    ["graveyard", "pine-fall.glb"],
    ["graveyard", "pine-fall-crooked.glb"],
  ],
  car: [
    ["car", "sedan.glb"],
    ["car", "suv.glb"],
    ["car", "van.glb"],
    ["car", "hatchback-sports.glb"],
    ["car", "taxi.glb"],
  ],
  train: [
    ["train", "train-locomotive-a.glb"],
    ["train", "train-locomotive-b.glb"],
    ["train", "train-diesel-a.glb"],
    ["train", "train-electric-city-a.glb"],
  ],
  boat: [
    ["watercraft", "boat-row-small.glb"],
    ["watercraft", "boat-fishing-small.glb"],
    ["watercraft", "boat-sail-a.glb"],
    ["watercraft", "boat-speed-a.glb"],
  ],
};

/** The characters, by the key the settings store persists; a person is
 * one cell tall, a pet a quarter. */
const ACTORS: Record<string, { pack: keyof typeof PACKS; files: readonly [key: string, file: string][]; size: number }> = {
  person: {
    pack: "mini_characters",
    size: 1,
    files: [
      ["female-1", "female-a.glb"],
      ["female-2", "female-b.glb"],
      ["female-3", "female-c.glb"],
      ["female-4", "female-d.glb"],
      ["female-5", "female-e.glb"],
      ["female-6", "female-f.glb"],
      ["male-1", "male-a.glb"],
      ["male-2", "male-b.glb"],
      ["male-3", "male-c.glb"],
      ["male-4", "male-d.glb"],
      ["male-5", "male-e.glb"],
      ["male-6", "male-f.glb"],
    ],
  },
  pet: {
    pack: "cube_pets",
    size: 0.25,
    files: [
      "beaver",
      "bee",
      "bunny",
      "cat",
      "caterpillar",
      "chick",
      "cow",
      "crab",
      "deer",
      "dog",
      "elephant",
      "fish",
      "fox",
      "giraffe",
      "hog",
      "koala",
      "lion",
      "monkey",
      "panda",
      "parrot",
      "penguin",
      "pig",
      "polar",
      "tiger",
    ].map((name) => [name, `${name}.glb`] as const),
  },
};

function ingested(path: string) {
  const row = INGESTED[path];
  if (!row) throw new Error(`palette: ${path} is not in ingested.ts — run the asset ingest`);
  return row;
}

function model_row(id: u32, kind: PaletteKind, name: string, group: string, pack: Pack, file: string, size: number): PaletteEntry {
  const model = ingested(`${pack.dir}/${file}`);
  const skin = ingested(`${pack.dir}/${pack.skin}`);
  return {
    id,
    kind,
    name,
    model_hash: fnv1a64_str(model.key),
    model_key: model.key,
    skin_key: skin.key,
    extent: model.extent ?? [0, 0, 0],
    size,
    ...KENNEY,
    group,
  };
}

function cube_rows(): PaletteEntry[] {
  const rows: PaletteEntry[] = [];
  const channel = (name: string, base: u32, kind: PaletteKind, types: Record<string, number>, colors: Record<number, string | null>) => {
    for (const [type_name, local] of Object.entries(types)) {
      rows.push({
        id: base + local,
        kind,
        name: `${name}.${type_name}`,
        model_hash: 0n,
        model_key: "",
        extent: [1, 1, 1],
        size: 1,
        license: "",
        author: "",
        color: colors[local] ?? undefined,
      });
    }
  };
  channel("common", 0, "cube", cube_types.common, cube_colors.common);
  channel("block", 0, "cube", cube_types.block, cube_colors.block);
  channel("land", CHANNEL_ID_BASE.land, "ground", cube_types.land, cube_colors.land);
  channel("water", CHANNEL_ID_BASE.water, "ground", cube_types.water, cube_colors.water);
  channel("structure", CHANNEL_ID_BASE.structure, "ground", cube_types.structure, cube_colors.structure);
  channel("transport", CHANNEL_ID_BASE.transport, "ground", cube_types.transport, cube_colors.transport);
  return rows;
}

function decor_rows(): PaletteEntry[] {
  const rows: PaletteEntry[] = [];
  // The placeable decor kinds, by the number the object store writes; a
  // kind's extent is the largest over its models.
  for (const [name, id] of Object.entries(decor_types)) {
    const extent: [number, number, number] = [0, 0, 0];
    for (const [pack, file] of MODELS[name] ?? []) {
      const model = ingested(`${PACKS[pack].dir}/${file}`).extent ?? [0, 0, 0];
      for (let i = 0; i < 3; i++) extent[i] = Math.max(extent[i], model[i]);
    }
    rows.push({ id, kind: "decor", name, model_hash: 0n, model_key: "", extent, size: 1, ...KENNEY, group: name });
  }
  let id = MODEL_ID_BASE;
  for (const [group, models] of Object.entries(MODELS)) {
    for (const [pack, file] of models) {
      rows.push(model_row(id++, "decor", `${group}:${file.replace(".glb", "")}`, group, PACKS[pack], file, 1));
    }
  }
  return rows;
}

function actor_rows(): PaletteEntry[] {
  const rows: PaletteEntry[] = [];
  let id = ACTOR_ID_BASE;
  for (const [group, { pack, files, size }] of Object.entries(ACTORS)) {
    for (const [key, file] of files) rows.push(model_row(id++, "actor", key, group, PACKS[pack], file, size));
  }
  return rows;
}

export const SHIPPED_PALETTE: readonly PaletteEntry[] = [...cube_rows(), ...decor_rows(), ...actor_rows()];

let $shipped: PaletteTable | null = null;

/** The shipped palette as the record, encoded once. */
export function shipped_palette(): PaletteTable {
  if (!$shipped) $shipped = new PaletteTable(encode_palette(SHIPPED_PALETTE));
  return $shipped;
}
