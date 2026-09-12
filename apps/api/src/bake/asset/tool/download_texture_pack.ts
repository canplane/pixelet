// @pxl/api/bake/asset/tool/download_texture_pack.ts
//
// Re-runnable, one-off tool — not part of the app build. For every entry in
// asset/cube/manifest.json: fetches the URL only if asset/cube/original/
// doesn't already have a valid copy from that same URL (the cached original
// is the resize source, so a re-run never re-fetches just to re-normalize),
// then always (re)normalizes to a TILE_SIZExTILE_SIZE RGBA PNG under
// asset/cube/{terrain,block}/. The client fetches those by content hash
// from the bucket, so a changed image is followed by ingest_asset.ts.
// Run with:
//   deno run -A apps/api/src/bake/asset/tool/download_texture_pack.ts
//
// - a failed URL is retried a few times, then reported by name
// - a source whose aspect ratio isn't square is rejected as a possible
//   multi-frame sheet rather than squashed to size
// - resize algorithm follows the entry's manifest "style": Lanczos3 (via
//   sharp) for "cute-painted-fantasy-v1", nearest-neighbor for "legacy"
// - partial alpha is flattened onto paletteHex (or the image's own
//   opaque-pixel average, for the one entry with no paletteHex)
// - every final file's SHA-256 is checked against every other's

import { PNG } from "npm:pngjs@7";
import sharp from "npm:sharp@0.33";
import { Buffer } from "node:buffer";

import { CUBE_ASSET_DIR, fetch_with_retry, flatten_manifest, load_manifest, run_pool, type WorkItem } from "./manifest.ts";

const RETRY_ATTEMPTS = 3;
const CONCURRENCY = 6;

/** Must match LAYER_TEX_SZ in apps/web/src/render/tex_array.ts (one shared DataArrayTexture, one size for every layer). */
const TILE_SIZE = 256;

function hex_to_rgb(hex: string): [number, number, number] {
  const h = hex.replace("#", "");
  return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
}

/** No unintended transparent pixels afterward: every alpha becomes 255. Uses
 * the manifest's own paletteHex as the flatten background where given (the
 * color that type is already known to read as), falling back to the
 * image's own opaque-pixel average only for the one entry with none. */
function flatten_alpha(data: Uint8Array, w: number, h: number, palette_hex: string | null): { data: Uint8Array; flattened: number } {
  let bg: [number, number, number];
  if (palette_hex) {
    bg = hex_to_rgb(palette_hex);
  } else {
    let r = 0, g = 0, b = 0, n = 0;
    for (let i = 0; i < w * h; i++) {
      const o = i * 4;
      if (data[o + 3] === 255) {
        r += data[o];
        g += data[o + 1];
        b += data[o + 2];
        n++;
      }
    }
    bg = n > 0 ? [Math.round(r / n), Math.round(g / n), Math.round(b / n)] : [200, 200, 200];
  }

  const out = new Uint8Array(data);
  let flattened = 0;
  for (let i = 0; i < w * h; i++) {
    const o = i * 4;
    const a = out[o + 3];
    if (a < 255) {
      flattened++;
      const t = a / 255;
      out[o] = Math.round(out[o] * t + bg[0] * (1 - t));
      out[o + 1] = Math.round(out[o + 1] * t + bg[1] * (1 - t));
      out[o + 2] = Math.round(out[o + 2] * t + bg[2] * (1 - t));
      out[o + 3] = 255;
    }
  }
  return { data: out, flattened };
}

/** Nearest-neighbor only — pixel-art clusters must stay crisp, not blur
 * into a bilinear smear. Only called on square sources (see
 * looks_like_sprite_sheet), so this never distorts aspect ratio. */
function nearest_resize(src: Uint8Array, sw: number, sh: number, dw: number, dh: number): Uint8Array {
  const dst = new Uint8Array(dw * dh * 4);
  for (let y = 0; y < dh; y++) {
    const sy = Math.min(sh - 1, Math.floor((y + 0.5) * (sh / dh)));
    for (let x = 0; x < dw; x++) {
      const sx = Math.min(sw - 1, Math.floor((x + 0.5) * (sw / dw)));
      const s_off = (sy * sw + sx) * 4;
      const d_off = (y * dw + x) * 4;
      dst[d_off] = src[s_off];
      dst[d_off + 1] = src[s_off + 1];
      dst[d_off + 2] = src[s_off + 2];
      dst[d_off + 3] = src[s_off + 3];
    }
  }
  return dst;
}

/** Real smoothing resample for the hand-painted "cute-painted-fantasy-v1"
 * sources (often 400x400+) — a Lanczos3 kernel band-limits before
 * downsampling, unlike nearest-neighbor (which just drops samples and
 * would alias fine painted detail into pixelation, the exact artifact
 * these sources need to avoid). Sharp's `fit: "fill"` matches
 * nearest_resize's own contract: stretch to exactly dw x dh, no
 * letterboxing — safe here because looks_like_sprite_sheet already
 * rejected any non-square source before this is ever called. */
async function lanczos_resize(src: Uint8Array, sw: number, sh: number, dw: number, dh: number): Promise<Uint8Array> {
  const out = await sharp(Buffer.from(src), { raw: { width: sw, height: sh, channels: 4 } })
    .resize(dw, dh, { kernel: "lanczos3", fit: "fill" })
    .raw()
    .toBuffer();
  return new Uint8Array(out.buffer, out.byteOffset, out.byteLength);
}

/** A single tile source should be square. Anything else is either a
 * multi-frame sheet or a bad crop — never silently stretched into a tile. */
function looks_like_sprite_sheet(w: number, h: number): boolean {
  if (w === h) return false;
  return Math.max(w, h) / Math.min(w, h) >= 1.2;
}

/** manifest "file" -> source URL that produced the cached original, last
 * time this tool wrote it. A manifest edit that repoints an existing file
 * at a *different* URL must force a re-fetch of the original — its on-disk
 * bytes alone can't reveal that the manifest moved on. */
type SourceState = Record<string, string>;

function state_path(): URL {
  return new URL(".download_state.json", CUBE_ASSET_DIR);
}

async function load_state(): Promise<SourceState> {
  try {
    return JSON.parse(await Deno.readTextFile(state_path()));
  } catch {
    return {};
  }
}

async function save_state(state: SourceState): Promise<void> {
  await Deno.writeTextFile(state_path(), JSON.stringify(state, Object.keys(state).sort(), 2) + "\n");
}

/** Reads the cached original from disk if the manifest's URL for this item
 * still matches what produced it; null (never fetched, or manifest moved
 * on) means the caller must fetch fresh. */
async function read_cached_original(item: WorkItem, state: SourceState): Promise<Uint8Array | null> {
  if (state[item.file] !== item.url) return null;
  try {
    const bytes = await Deno.readFile(item.originals_dest);
    if (bytes.length === 0) return null;
    PNG.sync.read(Buffer.from(bytes));
    return bytes;
  } catch {
    return null;
  }
}

type ItemResult =
  | { label: string; status: "cached" | "fetched"; dest: URL; sha256: string }
  | { label: string; status: "failed"; url: string; reason: string };

async function process_item(item: WorkItem, state: SourceState): Promise<ItemResult> {
  let raw = await read_cached_original(item, state);
  const fetched = raw === null;

  if (raw === null) {
    try {
      raw = await fetch_with_retry(item.url, RETRY_ATTEMPTS);
    } catch (e) {
      return { label: item.label, status: "failed", url: item.url, reason: `download failed: ${e instanceof Error ? e.message : String(e)}` };
    }
    await Deno.mkdir(new URL(".", item.originals_dest), { recursive: true });
    await Deno.writeFile(item.originals_dest, raw);
    state[item.file] = item.url;
  }

  let png: InstanceType<typeof PNG>;
  try {
    png = PNG.sync.read(Buffer.from(raw));
  } catch (e) {
    return { label: item.label, status: "failed", url: item.url, reason: `not a decodable PNG: ${e instanceof Error ? e.message : String(e)}` };
  }

  let rgba: Uint8Array = new Uint8Array(png.data.buffer, png.data.byteOffset, png.data.byteLength);
  let w = png.width, h = png.height;

  if (w !== TILE_SIZE || h !== TILE_SIZE) {
    if (looks_like_sprite_sheet(w, h)) {
      return { label: item.label, status: "failed", url: item.url, reason: `suspected sprite sheet / non-square source (${w}x${h}) — needs manual review, not auto-resized` };
    }
    rgba = item.style === "cute-painted-fantasy-v1" ? await lanczos_resize(rgba, w, h, TILE_SIZE, TILE_SIZE) : nearest_resize(rgba, w, h, TILE_SIZE, TILE_SIZE);
    w = h = TILE_SIZE;
  }

  const { data: flattened } = flatten_alpha(rgba, w, h, item.paletteHex);

  const out = new PNG({ width: TILE_SIZE, height: TILE_SIZE });
  out.data = Buffer.from(flattened);
  const encoded = PNG.sync.write(out);

  await Deno.mkdir(new URL(".", item.dest), { recursive: true });
  await Deno.writeFile(item.dest, encoded);

  // re-validate what actually landed on disk, not just the in-memory buffer
  const roundtrip = PNG.sync.read(Buffer.from(await Deno.readFile(item.dest)));
  if (roundtrip.width !== TILE_SIZE || roundtrip.height !== TILE_SIZE) {
    return { label: item.label, status: "failed", url: item.url, reason: `post-write validation failed: got ${roundtrip.width}x${roundtrip.height}` };
  }
  for (let i = 3; i < roundtrip.data.length; i += 4) {
    if (roundtrip.data[i] !== 255) {
      return { label: item.label, status: "failed", url: item.url, reason: "post-write validation failed: unflattened alpha remains" };
    }
  }

  const digest = await crypto.subtle.digest("SHA-256", encoded);
  const sha256 = Array.from(new Uint8Array(digest)).map((b) => b.toString(16).padStart(2, "0")).join("");

  return { label: item.label, status: fetched ? "fetched" : "cached", dest: item.dest, sha256 };
}

async function main() {
  const manifest = await load_manifest();
  const items = flatten_manifest(manifest);
  console.log(`[texture-pack] ${items.length} manifest entries to process`);

  const state = await load_state();
  const results = await run_pool(items, CONCURRENCY, (item) => process_item(item, state));

  const live_keys = new Set(items.map((item) => item.file));
  for (const key of Object.keys(state)) if (!live_keys.has(key)) delete state[key];
  await save_state(state);

  const ok = results.filter((r) => r.status !== "failed") as Extract<ItemResult, { status: "cached" | "fetched" }>[];
  const cached = ok.filter((r) => r.status === "cached");
  const fetched = ok.filter((r) => r.status === "fetched");
  const failed = results.filter((r) => r.status === "failed") as Extract<ItemResult, { status: "failed" }>[];

  console.log(`[texture-pack] used cached original (no network): ${cached.length}`);
  console.log(`[texture-pack] fetched from network: ${fetched.length}`);
  console.log(`[texture-pack] failed: ${failed.length}`);

  if (failed.length > 0) {
    console.log("\n[texture-pack] FAILED entries (not substituted with anything else):");
    for (const f of failed) console.log(`  - ${f.label}: ${f.reason}\n      ${f.url}`);
  }

  const by_hash = new Map<string, string[]>();
  for (const r of ok) {
    const arr = by_hash.get(r.sha256) ?? [];
    arr.push(r.label);
    by_hash.set(r.sha256, arr);
  }
  const collisions = Array.from(by_hash.entries()).filter(([, labels]) => labels.length > 1);
  if (collisions.length > 0) {
    console.log("\n[texture-pack] SHA-256 COLLISIONS between distinct manifest entries:");
    for (const [hash, labels] of collisions) console.log(`  - ${hash}: ${labels.join(", ")}`);
  }

  console.log(`\n[texture-pack] total valid files: ${ok.length} / ${items.length}`);

  if (failed.length > 0 || collisions.length > 0 || ok.length !== items.length) {
    console.log("[texture-pack] FAILED — see above.");
    Deno.exit(1);
  }
  console.log(`[texture-pack] OK — all ${items.length} entries present, valid, and distinct.`);
}

if (import.meta.main) {
  await main();
}
