// @pxl/shared/assets/cell/tools/manifest.test.ts
//
// Validates pixelet_ms2_texture_manifest.json's own shape and distinctness
// invariants — the same ones download_texture_pack.ts enforces at fetch
// time, run here as a real regression test so a future manifest edit that
// collapses two distinct types back onto one image (the exact anti-pattern
// this whole pack was built to avoid) fails CI instead of only showing up
// visually. Reads the manifest directly rather than importing
// terrain_images.ts/object_images.ts: those files import real .png assets,
// which only Vite's build pipeline (not plain `deno test`) can resolve.

import { assertEquals, assertNotEquals } from "@std/assert";
import { PNG } from "npm:pngjs@7";
import { Buffer } from "node:buffer";
import { flatten_manifest, load_manifest } from "./manifest.ts";

const EXPECTED_SIDE_TYPES = [
  "BARE",
  "GREEN_DEEP",
  "GREEN_LIGHT",
  "SPORTS",
  "SAND",
  "RESIDENTIAL",
  "COMMERCIAL",
  "INDUSTRIAL",
  "EDUCATION",
  "MEDICAL",
  "PUBLIC",
  "APRON",
  "HELIPAD",
].sort();

// The two source images at the center of the BARE/SPORTS side-texture audit
// (see terrain_images.ts's own GROUND_LAYER_SIDE_OVERRIDES_COZY comment):
// #28 is bare exposed soil with no grass cap — a visible seam against
// BARE's grassy top — and must never come back as an active mapping for
// any type. #30 is grass-capped soil, confirmed the better match for
// BARE's own top texture.
const SOURCE_28_EXPOSED_SOIL_URL = "https://image-assets.jianhua.art/public/1788082415653_ee1ee5.png";
const SOURCE_30_GRASS_CAPPED_URL = "https://image-assets.jianhua.art/public/1788082420916_1yy77b.png";

Deno.test("manifest - top has 32 entries across land/water/structure/transport", async () => {
  const manifest = await load_manifest();
  const counts = Object.fromEntries(Object.entries(manifest.terrain.top).map(([k, v]) => [k, Object.keys(v).length]));
  assertEquals(counts, { land: 14, water: 3, structure: 5, transport: 10 });
});

Deno.test("manifest - side overrides exactly these 13 land types, no more, no fewer", async () => {
  const manifest = await load_manifest();
  assertEquals(Object.keys(manifest.terrain.side).sort(), EXPECTED_SIDE_TYPES);
});

Deno.test("manifest - objects.block has all 15 colors", async () => {
  const manifest = await load_manifest();
  assertEquals(Object.keys(manifest.objects.block).length, 15);
});

Deno.test("manifest - 60 entries total (32 top + 13 side + 15 objects)", async () => {
  const manifest = await load_manifest();
  const items = flatten_manifest(manifest);
  assertEquals(items.length, 60);
});

Deno.test("manifest - no two distinct entries share a source URL", async () => {
  const manifest = await load_manifest();
  const items = flatten_manifest(manifest);
  const by_url = new Map<string, string[]>();
  for (const item of items) by_url.set(item.url, [...(by_url.get(item.url) ?? []), item.label]);
  const collisions = [...by_url.entries()].filter(([, labels]) => labels.length > 1);
  assertEquals(collisions, [], `distinct manifest entries must never share a source image: ${JSON.stringify(collisions)}`);
});

Deno.test("manifest - no two distinct entries share a destination filename", async () => {
  const manifest = await load_manifest();
  const items = flatten_manifest(manifest);
  const by_file = new Map<string, string[]>();
  for (const item of items) by_file.set(item.file, [...(by_file.get(item.file) ?? []), item.label]);
  const collisions = [...by_file.entries()].filter(([, labels]) => labels.length > 1);
  assertEquals(collisions, [], `distinct manifest entries must never target the same file: ${JSON.stringify(collisions)}`);
});

Deno.test("manifest - SPORTS has its own dedicated side asset, not reused from BARE/GREEN_DEEP/GREEN_LIGHT", async () => {
  const manifest = await load_manifest();
  const sports_url = manifest.terrain.side.SPORTS.url;
  assertNotEquals(sports_url, manifest.terrain.side.BARE.url);
  assertNotEquals(sports_url, manifest.terrain.side.GREEN_DEEP.url);
  assertNotEquals(sports_url, manifest.terrain.side.GREEN_LIGHT.url);
  assertNotEquals(sports_url, SOURCE_28_EXPOSED_SOIL_URL);
});

Deno.test("manifest - BARE's side is the grass-capped asset (#30), never the bare-soil one (#28)", async () => {
  const manifest = await load_manifest();
  assertEquals(manifest.terrain.side.BARE.url, SOURCE_30_GRASS_CAPPED_URL);
  assertNotEquals(manifest.terrain.side.BARE.url, SOURCE_28_EXPOSED_SOIL_URL);
});

Deno.test("manifest - #28 (bare exposed soil) is not assigned to any active type", async () => {
  const manifest = await load_manifest();
  const items = flatten_manifest(manifest);
  const assigned = items.find((item) => item.url === SOURCE_28_EXPOSED_SOIL_URL);
  assertEquals(assigned, undefined, `#28 must stay unassigned, not silently reused: ${JSON.stringify(assigned)}`);
});

Deno.test("manifest - every entry's destination file exists on disk as a valid, opaque 64x64 PNG", async () => {
  const manifest = await load_manifest();
  const items = flatten_manifest(manifest);
  for (const item of items) {
    const bytes = await Deno.readFile(item.dest).catch(() => {
      throw new Error(`missing file for ${item.label}: ${item.dest}`);
    });
    const png = PNG.sync.read(Buffer.from(bytes));
    assertEquals(png.width, 64, `${item.label} width`);
    assertEquals(png.height, 64, `${item.label} height`);
    for (let i = 3; i < png.data.length; i += 4) {
      assertEquals(png.data[i], 255, `${item.label} has an unflattened transparent pixel`);
    }
  }
});
