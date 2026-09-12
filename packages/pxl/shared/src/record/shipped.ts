// @pxl/shared/record/shipped.ts

/**
 * The palette this build ships: every cube and ground type the bake and
 * the object store use, and every decor model, as rows of the palette
 * record. What the constants in spec/cell said is said here as data, and
 * the constants keep their numbers so that nothing stored has to move.
 *
 * Ids. A placeable value — a block colour, AIR, a decor kind — keeps the
 * number the object store already writes (spec/cell), which is under 256.
 * A terrain channel's types overlap each other by number, since each is
 * its own lane, so here each channel is offset into its own range. A
 * model is a row from MODEL_ID_BASE, one per glb, grouped by the scatter
 * kind that uses it; a kind's variant order is this file's row order,
 * which the bundle that lists the glb URLs checks against.
 *
 * Extents are the glb bounds in ENU (x, y, z up), read by the same bounds
 * pass an ingest would run. Until the models live in the bucket by
 * content hash, `model_key` is the asset path and `model_hash` is 0.
 */

import type { u32 } from "@xpute/core/kit/word.ts";

import { cube_colors, cube_types, decor_types } from "../spec/cell/mod.ts";
import { encode_palette, type PaletteEntry, type PaletteKind, PaletteTable } from "./palette.ts";

/** Terrain channels' types, offset so a global id is unique. */
export const CHANNEL_ID_BASE = { land: 0x100, water: 0x200, structure: 0x300, transport: 0x400 } as const;
export const MODEL_ID_BASE: u32 = 0x1000;

const KENNEY = { license: "CC0-1.0", author: "Kenney" } as const;

/** A model row's bounds in ENU units, from the glb: x, y, z up. */
type Model = [file: string, x: number, y: number, z: number];

const MODELS: Record<string, readonly Model[]> = {
  // Order is the variant order of assets/decor/trees.ts then
  // graveyard_trees.ts, cars.ts, trains.ts, boats.ts.
  tree: [
    ["kenney_fantasy_town/tree.glb", 1.02, 1.02, 2.41],
    ["kenney_fantasy_town/tree-high.glb", 1.02, 1.02, 2.97],
    ["kenney_fantasy_town/tree-crooked.glb", 1.02, 1.02, 2.41],
    ["kenney_fantasy_town/tree-high-round.glb", 1.02, 1.02, 2.75],
    ["kenney_fantasy_town/tree-high-crooked.glb", 1.02, 1.02, 2.97],
    ["kenney_graveyard_kit/pine.glb", 1.18, 1.33, 2.3],
    ["kenney_graveyard_kit/pine-crooked.glb", 1.18, 1.33, 2.11],
    ["kenney_graveyard_kit/pine-fall.glb", 1.18, 1.33, 2.3],
    ["kenney_graveyard_kit/pine-fall-crooked.glb", 1.18, 1.33, 2.11],
  ],
  car: [
    ["kenney_car_kit/sedan.glb", 1.5, 2.55, 1.45],
    ["kenney_car_kit/suv.glb", 1.5, 2.55, 1.4],
    ["kenney_car_kit/van.glb", 1.5, 2.75, 1.45],
    ["kenney_car_kit/hatchback-sports.glb", 1.3, 2.85, 1.25],
    ["kenney_car_kit/taxi.glb", 1.5, 2.75, 1.65],
  ],
  train: [
    ["kenney_train_kit/train-locomotive-a.glb", 1.43, 2.6, 2.02],
    ["kenney_train_kit/train-locomotive-b.glb", 1.32, 2.6, 1.93],
    ["kenney_train_kit/train-diesel-a.glb", 1.2, 2.4, 1.96],
    ["kenney_train_kit/train-electric-city-a.glb", 1.1, 2.4, 2.04],
  ],
  boat: [
    ["kenney_watercraft_pack/boat-row-small.glb", 2.75, 2.37, 0.79],
    ["kenney_watercraft_pack/boat-fishing-small.glb", 1.78, 3.87, 2.6],
    ["kenney_watercraft_pack/boat-sail-a.glb", 1.78, 3.77, 3.87],
    ["kenney_watercraft_pack/boat-speed-a.glb", 1.78, 3.37, 1.5],
  ],
};

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
  // The placeable decor kinds, by the number the object store writes.
  for (const [name, id] of Object.entries(decor_types)) {
    const models = MODELS[name] ?? [];
    const extent: [number, number, number] = [0, 0, 0];
    for (const [, x, y, z] of models) {
      extent[0] = Math.max(extent[0], x);
      extent[1] = Math.max(extent[1], y);
      extent[2] = Math.max(extent[2], z);
    }
    rows.push({ id, kind: "decor", name, model_hash: 0n, model_key: "", extent, size: 1, ...KENNEY, group: name });
  }
  let id = MODEL_ID_BASE;
  for (const [group, models] of Object.entries(MODELS)) {
    for (const [file, x, y, z] of models) {
      rows.push({ id: id++, kind: "decor", name: `${group}:${file.split("/").pop()!.replace(".glb", "")}`, model_hash: 0n, model_key: `decor/${file}`, extent: [x, y, z], size: 1, ...KENNEY, group });
    }
  }
  return rows;
}

export const SHIPPED_PALETTE: readonly PaletteEntry[] = [...cube_rows(), ...decor_rows()];

let $shipped: PaletteTable | null = null;

/** The shipped palette as the record, encoded once. */
export function shipped_palette(): PaletteTable {
  if (!$shipped) $shipped = new PaletteTable(encode_palette(SHIPPED_PALETTE));
  return $shipped;
}
