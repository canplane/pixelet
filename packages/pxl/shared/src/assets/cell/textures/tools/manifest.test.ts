// @pxl/shared/assets/cell/textures/tools/manifest.test.ts
//
// Validates manifest.json's own shape and distinctness
// invariants — the same ones download_texture_pack.ts enforces at fetch
// time, run here as a real regression test so a future manifest edit that
// collapses two distinct types back onto one image (the exact anti-pattern
// this whole pack was built to avoid) fails CI instead of only showing up
// visually. Reads the manifest directly rather than importing
// ../terrain/wire.ts, ../objects/wire.ts: those files import real .png assets,
// which only Vite's build pipeline (not plain `deno test`) can resolve.

import { assertEquals, assertNotEquals } from "@std/assert";
import { PNG } from "npm:pngjs@7";
import { Buffer } from "node:buffer";
import { flatten_manifest, load_manifest } from "./manifest.ts";
import { terrain_types } from "@pxl/shared/spec/cell/terrain.ts";

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

// The source image at the center of the original BARE/SPORTS side-texture
// audit (see ../terrain/wire.ts's own `side` field
// comment): bare exposed soil with no grass cap — a visible seam against
// BARE's grassy top — and must never come back as an active mapping for
// any type, regardless of which pack generation is currently active.
const SOURCE_28_EXPOSED_SOIL_URL = "https://image-assets.jianhua.art/public/1788082415653_ee1ee5.png";

// BARE's side in the current (cute-painted-fantasy-v1) pack generation —
// pinned so a future manifest edit that quietly repoints BARE's side back
// at exposed soil (or anything else) fails loudly here instead of only
// showing up as a visual seam.
const SOURCE_BARE_SIDE_CUTE_PAINTED_URL = "https://image-assets.jianhua.art/public/1788243271391_ajtb22.png";

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

Deno.test("manifest - all 60 entries are cute-painted-fantasy-v1, no legacy entries remain", async () => {
  const manifest = await load_manifest();
  const items = flatten_manifest(manifest);
  const cute_painted = items.filter((item) => item.style === "cute-painted-fantasy-v1");
  const legacy = items.filter((item) => item.style === "legacy");
  assertEquals(cute_painted.length, 60, `cute-painted entries: ${JSON.stringify(cute_painted.map((i) => i.label))}`);
  assertEquals(legacy.length, 0, `legacy entries: ${JSON.stringify(legacy.map((i) => i.label))}`);
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

Deno.test("manifest - BARE's side is the current cute-painted asset, never the old bare-soil one", async () => {
  const manifest = await load_manifest();
  assertEquals(manifest.terrain.side.BARE.url, SOURCE_BARE_SIDE_CUTE_PAINTED_URL);
  assertEquals(manifest.terrain.side.BARE.style, "cute-painted-fantasy-v1");
  assertNotEquals(manifest.terrain.side.BARE.url, SOURCE_28_EXPOSED_SOIL_URL);
});

Deno.test("manifest - #28 (bare exposed soil) is not assigned to any active type", async () => {
  const manifest = await load_manifest();
  const items = flatten_manifest(manifest);
  const assigned = items.find((item) => item.url === SOURCE_28_EXPOSED_SOIL_URL);
  assertEquals(assigned, undefined, `#28 must stay unassigned, not silently reused: ${JSON.stringify(assigned)}`);
});

// Must match TILE_SIZE in download_texture_pack.ts — see that file's own
// comment for why the whole pack shares one output size.
const TILE_SIZE = 256;

Deno.test("manifest - every entry's destination file exists on disk as a valid, opaque 256x256 PNG", async () => {
  const manifest = await load_manifest();
  const items = flatten_manifest(manifest);
  for (const item of items) {
    const bytes = await Deno.readFile(item.dest).catch(() => {
      throw new Error(`missing file for ${item.label}: ${item.dest}`);
    });
    const png = PNG.sync.read(Buffer.from(bytes));
    assertEquals(png.width, TILE_SIZE, `${item.label} width`);
    assertEquals(png.height, TILE_SIZE, `${item.label} height`);
    for (let i = 3; i < png.data.length; i += 4) {
      assertEquals(png.data[i], 255, `${item.label} has an unflattened transparent pixel`);
    }
  }
});

Deno.test("manifest - no two active on-disk files are byte-identical (SHA-256)", async () => {
  const manifest = await load_manifest();
  const items = flatten_manifest(manifest);
  const by_hash = new Map<string, string[]>();
  for (const item of items) {
    const bytes = await Deno.readFile(item.dest);
    const digest = await crypto.subtle.digest("SHA-256", bytes);
    const hash = Array.from(new Uint8Array(digest)).map((b) => b.toString(16).padStart(2, "0")).join("");
    by_hash.set(hash, [...(by_hash.get(hash) ?? []), item.label]);
  }
  const collisions = [...by_hash.entries()].filter(([, labels]) => labels.length > 1);
  assertEquals(collisions, [], `distinct active entries must never end up byte-identical on disk: ${JSON.stringify(collisions)}`);
});

// terrain_types.ts is a plain spec module (JSON palette imports only, no
// PNG/GLB assets) — safe to import directly here, unlike ../terrain/wire.ts.
// A texture-pack swap must never renumber the actual type IDs baked into
// stored tiles; this is a tripwire, not exhaustive.
Deno.test("manifest integration - terrain type IDs are untouched by this pack's own image swap", () => {
  assertEquals(terrain_types.land.BARE, 1);
  assertEquals(terrain_types.land.GREEN_DEEP, 10);
  assertEquals(terrain_types.land.GREEN_LIGHT, 11);
  assertEquals(terrain_types.land.SAND, 12);
  assertEquals(terrain_types.land.HELIPAD, 31);
  assertEquals(terrain_types.water.RIVER, 1);
  assertEquals(terrain_types.water.LAKE, 2);
  assertEquals(terrain_types.water.WETLAND, 3);
});
