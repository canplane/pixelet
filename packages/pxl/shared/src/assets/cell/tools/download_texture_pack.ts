// @pxl/shared/assets/cell/tools/download_texture_pack.ts
//
// Re-runnable, one-off tool (same role as apps/api's bake/terrain/tools/*)
// — not part of the app build. Downloads every image URL in the repo-root
// pixelet_ms2_texture_manifest.json, normalizes each to a standalone 64x64
// RGBA PNG, and writes it under ../textures/{terrain,objects}/. Run
// with:
//   deno run -A packages/pxl/shared/src/assets/cell/tools/download_texture_pack.ts
//
// Safety properties the manifest's own integration spec asked for:
// - already-valid files are never re-fetched (idempotent re-run) — tracked
//   by source URL (../textures/.download_state.json), not just by the
//   destination file looking like a valid 64x64 PNG, so a manifest edit
//   that repoints an existing filename at a different URL is still picked
//   up on the next run instead of leaving the old image in place forever
// - a failed URL is retried a few times, then reported by name — never
//   silently swapped for a different image
// - a source image whose aspect ratio isn't square is treated as a
//   possible multi-frame sheet and rejected rather than squashed to 64x64
// - partial alpha is flattened onto the manifest's own paletteHex (or, for
//   the one entry with no paletteHex, the image's own opaque-pixel
//   average) so no transparent pixel can render as unintended black
// - every final file's SHA-256 is checked against every other file's, so
//   two distinct manifest entries can never silently end up byte-identical

import { PNG } from "npm:pngjs@7";
import { Buffer } from "node:buffer";

import { flatten_manifest, load_manifest, type WorkItem } from "./manifest.ts";

const RETRY_ATTEMPTS = 3;
const CONCURRENCY = 6;

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

/** A single 64x64 tile should be square. Anything else is either a
 * multi-frame sheet or a bad crop — never silently stretched into a tile. */
function looks_like_sprite_sheet(w: number, h: number): boolean {
  if (w === h) return false;
  return Math.max(w, h) / Math.min(w, h) >= 1.2;
}

async function fetch_with_retry(url: string, attempts: number): Promise<Uint8Array> {
  let last_err: unknown;
  for (let i = 1; i <= attempts; i++) {
    try {
      const res = await fetch(url);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const buf = new Uint8Array(await res.arrayBuffer());
      if (buf.length === 0) throw new Error("empty response body");
      return buf;
    } catch (e) {
      last_err = e;
      if (i < attempts) await new Promise((r) => setTimeout(r, 400 * i));
    }
  }
  throw last_err instanceof Error ? last_err : new Error(String(last_err));
}

/** dest path (relative to ../textures/) -> source URL that actually produced
 * it, last time this tool wrote it. A manifest edit that repoints an
 * existing filename at a *different* URL (exactly what happened correcting
 * BARE/SPORTS's side textures) must force a re-download — the on-disk file
 * alone still "looks valid" (right dimensions, fully opaque), so without
 * this record read_existing_if_valid would keep the now-stale old image
 * forever instead of ever refetching the corrected one. */
type SourceState = Record<string, string>;

function state_path(): URL {
  return new URL("../textures/.download_state.json", import.meta.url);
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

function dest_key(item: Pick<WorkItem, "dest">): string {
  return decodeURIComponent(new URL(item.dest).pathname.split("/textures/")[1]);
}

async function read_existing_if_valid(item: WorkItem, state: SourceState): Promise<boolean> {
  if (state[dest_key(item)] !== item.url) return false;
  try {
    const bytes = await Deno.readFile(item.dest);
    const png = PNG.sync.read(Buffer.from(bytes));
    if (png.width !== 64 || png.height !== 64) return false;
    for (let i = 3; i < png.data.length; i += 4) {
      if (png.data[i] !== 255) return false; // stale pre-flatten file — redo it
    }
    return true;
  } catch {
    return false;
  }
}

type ItemResult =
  | { label: string; status: "skipped_valid"; dest: URL }
  | { label: string; status: "written"; dest: URL; sha256: string }
  | { label: string; status: "failed"; url: string; reason: string };

async function process_item(item: WorkItem, state: SourceState): Promise<ItemResult> {
  if (await read_existing_if_valid(item, state)) {
    return { label: item.label, status: "skipped_valid", dest: item.dest };
  }

  let raw: Uint8Array;
  try {
    raw = await fetch_with_retry(item.url, RETRY_ATTEMPTS);
  } catch (e) {
    return { label: item.label, status: "failed", url: item.url, reason: `download failed: ${e instanceof Error ? e.message : String(e)}` };
  }

  let png: InstanceType<typeof PNG>;
  try {
    png = PNG.sync.read(Buffer.from(raw));
  } catch (e) {
    return { label: item.label, status: "failed", url: item.url, reason: `not a decodable PNG: ${e instanceof Error ? e.message : String(e)}` };
  }

  let rgba: Uint8Array = new Uint8Array(png.data.buffer, png.data.byteOffset, png.data.byteLength);
  let w = png.width, h = png.height;

  if (w !== 64 || h !== 64) {
    if (looks_like_sprite_sheet(w, h)) {
      return { label: item.label, status: "failed", url: item.url, reason: `suspected sprite sheet / non-square source (${w}x${h}) — needs manual review, not auto-resized` };
    }
    rgba = nearest_resize(rgba, w, h, 64, 64);
    w = h = 64;
  }

  const { data: flattened } = flatten_alpha(rgba, w, h, item.paletteHex);

  const out = new PNG({ width: 64, height: 64 });
  out.data = Buffer.from(flattened);
  const encoded = PNG.sync.write(out);

  await Deno.mkdir(new URL(".", item.dest), { recursive: true });
  await Deno.writeFile(item.dest, encoded);

  // re-validate what actually landed on disk, not just the in-memory buffer
  const roundtrip = PNG.sync.read(Buffer.from(await Deno.readFile(item.dest)));
  if (roundtrip.width !== 64 || roundtrip.height !== 64) {
    return { label: item.label, status: "failed", url: item.url, reason: `post-write validation failed: got ${roundtrip.width}x${roundtrip.height}` };
  }
  for (let i = 3; i < roundtrip.data.length; i += 4) {
    if (roundtrip.data[i] !== 255) {
      return { label: item.label, status: "failed", url: item.url, reason: "post-write validation failed: unflattened alpha remains" };
    }
  }

  const digest = await crypto.subtle.digest("SHA-256", encoded);
  const sha256 = Array.from(new Uint8Array(digest)).map((b) => b.toString(16).padStart(2, "0")).join("");

  state[dest_key(item)] = item.url;

  return { label: item.label, status: "written", dest: item.dest, sha256 };
}

async function run_pool<T, R>(items: T[], concurrency: number, fn: (item: T) => Promise<R>): Promise<R[]> {
  const results: R[] = new Array(items.length);
  let next = 0;
  async function worker() {
    while (true) {
      const i = next++;
      if (i >= items.length) return;
      results[i] = await fn(items[i]);
    }
  }
  await Promise.all(Array.from({ length: Math.min(concurrency, items.length) }, worker));
  return results;
}

async function main() {
  const manifest = await load_manifest();
  const items = flatten_manifest(manifest);
  console.log(`[texture-pack] ${items.length} manifest entries to process`);

  const state = await load_state();
  const results = await run_pool(items, CONCURRENCY, (item) => process_item(item, state));

  // Drop state for any key no longer in the manifest (e.g. a type whose side
  // override was removed) — an orphan entry here is harmless but stale.
  const live_keys = new Set(items.map((item) => dest_key(item)));
  for (const key of Object.keys(state)) if (!live_keys.has(key)) delete state[key];
  await save_state(state);

  const skipped = results.filter((r) => r.status === "skipped_valid");
  const written = results.filter((r) => r.status === "written") as Extract<ItemResult, { status: "written" }>[];
  const failed = results.filter((r) => r.status === "failed") as Extract<ItemResult, { status: "failed" }>[];

  console.log(`[texture-pack] skipped (already valid): ${skipped.length}`);
  console.log(`[texture-pack] downloaded + normalized: ${written.length}`);
  console.log(`[texture-pack] failed: ${failed.length}`);

  if (failed.length > 0) {
    console.log("\n[texture-pack] FAILED entries (not substituted with anything else):");
    for (const f of failed) console.log(`  - ${f.label}: ${f.reason}\n      ${f.url}`);
  }

  // Distinct-entry SHA-256 collision check — only meaningful across items
  // actually confirmed on disk this run; re-run to also cover skipped ones.
  const by_hash = new Map<string, string[]>();
  for (const w of written) {
    const arr = by_hash.get(w.sha256) ?? [];
    arr.push(w.label);
    by_hash.set(w.sha256, arr);
  }
  const collisions = Array.from(by_hash.entries()).filter(([, labels]) => labels.length > 1);
  if (collisions.length > 0) {
    console.log("\n[texture-pack] SHA-256 COLLISIONS between distinct manifest entries:");
    for (const [hash, labels] of collisions) console.log(`  - ${hash}: ${labels.join(", ")}`);
  }

  const total_valid = skipped.length + written.length;
  console.log(`\n[texture-pack] total valid files: ${total_valid} / ${items.length}`);

  if (failed.length > 0 || collisions.length > 0 || total_valid !== items.length) {
    console.log("[texture-pack] FAILED — see above.");
    Deno.exit(1);
  }
  console.log(`[texture-pack] OK — all ${items.length} entries present, valid, and distinct.`);
}

if (import.meta.main) {
  await main();
}
