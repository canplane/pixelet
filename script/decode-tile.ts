#!/usr/bin/env -S deno run -A
// A baked terrain page, read back the way the kernel reads it.
//
//   deno run -A script/decode-tile.ts 6235330563538944            # one page: summary + PNGs
//   deno run -A script/decode-tile.ts 6235330563538944 --out /tmp/page
//   deno run -A script/decode-tile.ts out/terrain/v3/_ocean       # a path works too
//   deno run -A script/decode-tile.ts --all                       # every page under out/terrain/<version>, checked
//   deno run -A script/decode-tile.ts --all --version v3
//
// A page is `[ele, [land, water, structure, transport]]` in XTP
// (crates/pxl/spec/src/bake/format.rs writes it, `decode_terrain` reads it),
// and this walks it with the same TreeReader the client uses
// (@xpute/core/wire/xtp), so a page this cannot read is one the client
// cannot either. What it checks past the reader is the bake's own contract:
// the shape (one i16 raster of ELE_PADDED_SPAN², four u8 planes of
// PAGE_CELL_SPAN²) and that every value in a plane is a type its channel
// gives out (spec/pxl/spec/cell/cube_type.json). A bake that drifts in either
// is caught here before it is published, which is what `--all` after
// `deno task bake:tiles` is for.
//
// For one page it also writes `heights.png` (the raster stretched to its own
// range, so a flat page is flat grey) and one PNG a plane in the palette's
// colours (spec/pxl/spec/cell/color.json), with `void` left black and a value
// the channel does not give out in magenta. The picture is for the eye; the
// summary is the check.

import { basename, join } from "@std/path";

import type { I16Array, U8Array } from "@xpute/core/abi/array.ts";
import { round } from "@xpute/core/math/scalar.ts";
import { SequenceType, SpecialType, TreeReader } from "@xpute/core/wire/xtp/mod.ts";

import { cube_types } from "../apps/web/src/catalog/cell/cube_type.spec.ts";
import color from "../apps/web/src/catalog/cell/color.spec.ts";

const root = new URL("..", import.meta.url).pathname;

// pxl-spec/bake/constant.rs. The Rust is the one place these are decided;
// this reads the numbers from the page and says which one it expected.
const PAGE_CELL_SPAN = 1024;
const ELE_PADDED_SPAN = 68;

const CHANNELS = ["land", "water", "structure", "transport"] as const;
type Channel = (typeof CHANNELS)[number];

// ---- arguments ----

const args = [...Deno.args];
function flag(name: string): string | undefined {
  const i = args.indexOf(name);
  if (i < 0) return undefined;
  const [, value] = args.splice(i, 2);
  return value;
}
const version = flag("--version") ?? "v3";
const outDir = flag("--out");
const all = args.includes("--all");
if (all) args.splice(args.indexOf("--all"), 1);
const target = args[0];

const pages = join(root, "out/terrain", version);

// ---- reading ----

async function readPage(path: string): Promise<U8Array> {
  const raw = await Deno.readFile(path);
  // The bake stores a page gzipped (manifest.json's `content_encoding`), and
  // a page fetched from a deployment arrives inflated; both are accepted.
  if (raw.length >= 2 && raw[0] === 0x1f && raw[1] === 0x8b) {
    const inflated = new Blob([raw]).stream().pipeThrough(new DecompressionStream("gzip"));
    return new Uint8Array(await new Response(inflated).arrayBuffer());
  }
  return raw;
}

interface Page {
  ele: I16Array;
  planes: Record<Channel, U8Array>;
}

/** The page as the kernel reads it, or a thrown Error naming what is wrong. */
function decode(pkt: U8Array): Page {
  const tree = new TreeReader(pkt).read_branch();
  if (tree.len !== 2) throw new Error(`root branch has ${tree.len} children, expected 2 [ele, ground]`);

  const ele = tree.at(0);
  if (ele.type !== SequenceType.I16_ARRAY) throw new Error(`ele is type 0x${ele.type.toString(16)}, expected I16_ARRAY`);
  const heights = ele.get<I16Array>();
  if (heights.length !== ELE_PADDED_SPAN * ELE_PADDED_SPAN) throw new Error(`ele has ${heights.length} samples, expected ${ELE_PADDED_SPAN}² = ${ELE_PADDED_SPAN * ELE_PADDED_SPAN}`);

  const ground = tree.at(1);
  if (ground.type !== SpecialType.BRANCH) throw new Error(`ground is type 0x${ground.type.toString(16)}, expected a branch`);
  const g = ground.as_branch();
  if (g.len !== CHANNELS.length) throw new Error(`ground has ${g.len} planes, expected ${CHANNELS.length} [${CHANNELS.join(", ")}]`);
  const planes = {} as Record<Channel, U8Array>;
  CHANNELS.forEach((ch, i) => {
    const node = g.at(i);
    if (node.type !== SequenceType.U8_ARRAY) throw new Error(`${ch} is type 0x${node.type.toString(16)}, expected U8_ARRAY`);
    const plane = node.get<U8Array>();
    if (plane.length !== PAGE_CELL_SPAN * PAGE_CELL_SPAN) throw new Error(`${ch} has ${plane.length} cells, expected ${PAGE_CELL_SPAN}² = ${PAGE_CELL_SPAN * PAGE_CELL_SPAN}`);
    planes[ch] = plane;
  });

  // Nothing may follow: a page is exactly its two children, and bytes past
  // them are a writer that disagrees with the reader.
  const payload = new DataView(pkt.buffer, pkt.byteOffset, pkt.byteLength).getUint32(8, true);
  if (payload + 16 !== pkt.byteLength) throw new Error(`packet is ${pkt.byteLength} bytes, header says ${payload + 16}`);

  return { ele: heights, planes };
}

// ---- checking ----

/** `value -> name` for a channel: what the catalog gives out, plus `void`. */
function namesOf(ch: Channel): Map<number, string> {
  const names = new Map<number, string>([[cube_types.common.void, "void"]]);
  for (const [name, value] of Object.entries(cube_types[ch])) names.set(value, name);
  return names;
}

interface Summary {
  ele: { min: number; max: number; mean: number; flat: boolean };
  planes: Record<Channel, { counts: Record<string, number>; unknown: Record<number, number> }>;
}

function summarize(page: Page): Summary {
  let min = Infinity, max = -Infinity, sum = 0;
  for (const h of page.ele) {
    if (h < min) min = h;
    if (h > max) max = h;
    sum += h;
  }
  const planes = {} as Summary["planes"];
  for (const ch of CHANNELS) {
    const names = namesOf(ch);
    const hist = new Uint32Array(256);
    for (const v of page.planes[ch]) hist[v]++;
    const counts: Record<string, number> = {};
    const unknown: Record<number, number> = {};
    hist.forEach((n, v) => {
      if (n === 0) return;
      const name = names.get(v);
      if (name === undefined) unknown[v] = n;
      else counts[name] = n;
    });
    planes[ch] = { counts, unknown };
  }
  return { ele: { min, max, mean: sum / page.ele.length, flat: min === max }, planes };
}

/** What is wrong with a page that decoded, or nothing. */
function faults(s: Summary): string[] {
  const out: string[] = [];
  for (const ch of CHANNELS) {
    const unknown = Object.entries(s.planes[ch].unknown);
    if (unknown.length > 0) out.push(`${ch}: value(s) the channel does not give out — ${unknown.map(([v, n]) => `${v}×${n}`).join(", ")}`);
  }
  return out;
}

// ---- PNG ----

const CRC_TABLE = new Uint32Array(256).map((_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});

function crc32(bytes: U8Array): number {
  let c = 0xffffffff;
  for (const b of bytes) c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type: string, data: U8Array): U8Array {
  const out = new Uint8Array(12 + data.length);
  const view = new DataView(out.buffer);
  view.setUint32(0, data.length);
  out.set(new TextEncoder().encode(type), 4);
  out.set(data, 8);
  view.setUint32(8 + data.length, crc32(out.subarray(4, 8 + data.length)));
  return out;
}

/** A PNG of `width`×`height` pixels, `channels` bytes each (1 grey, 3 RGB). */
async function png(width: number, height: number, channels: 1 | 3, pixels: U8Array): Promise<U8Array> {
  const stride = width * channels;
  const raw = new Uint8Array((stride + 1) * height);
  for (let y = 0; y < height; y++) raw.set(pixels.subarray(y * stride, (y + 1) * stride), y * (stride + 1) + 1);
  const deflated = new Uint8Array(await new Response(new Blob([raw]).stream().pipeThrough(new CompressionStream("deflate"))).arrayBuffer());
  const ihdr = new Uint8Array(13);
  const v = new DataView(ihdr.buffer);
  v.setUint32(0, width);
  v.setUint32(4, height);
  ihdr[8] = 8; // bit depth
  ihdr[9] = channels === 1 ? 0 : 2; // greyscale | truecolour
  const parts = [new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk("IHDR", ihdr), chunk("IDAT", deflated), chunk("IEND", new Uint8Array(0))];
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}

function heightsPng(ele: I16Array, s: Summary): Promise<U8Array> {
  const span = s.ele.max - s.ele.min;
  const grey = new Uint8Array(ele.length);
  for (let i = 0; i < ele.length; i++) grey[i] = span === 0 ? 128 : round(((ele[i] - s.ele.min) / span) * 255);
  return png(ELE_PADDED_SPAN, ELE_PADDED_SPAN, 1, grey);
}

/** The palette's colour for every value of a channel; magenta for one it
 * lacks, black for void. */
function paletteOf(ch: Channel): Uint32Array {
  const rgb = new Uint32Array(256).fill(0xff00ff);
  rgb[cube_types.common.void] = 0x000000;
  const swatches = color[ch] as Record<string, { rgb: number }>;
  for (const [name, value] of Object.entries(cube_types[ch])) {
    const swatch = swatches[name];
    if (swatch !== undefined) rgb[value] = swatch.rgb;
  }
  return rgb;
}

function planePng(ch: Channel, plane: U8Array): Promise<U8Array> {
  const palette = paletteOf(ch);
  const px = new Uint8Array(plane.length * 3);
  for (let i = 0; i < plane.length; i++) {
    const c = palette[plane[i]];
    px[i * 3] = c >>> 16;
    px[i * 3 + 1] = (c >>> 8) & 0xff;
    px[i * 3 + 2] = c & 0xff;
  }
  return png(PAGE_CELL_SPAN, PAGE_CELL_SPAN, 3, px);
}

// ---- driver ----

if (all) {
  let count = 0;
  const bad: string[] = [];
  for await (const entry of Deno.readDir(pages)) {
    if (!entry.isFile) continue;
    count++;
    const path = join(pages, entry.name);
    try {
      const f = faults(summarize(decode(await readPage(path))));
      if (f.length > 0) bad.push(`${entry.name}: ${f.join("; ")}`);
    } catch (e) {
      bad.push(`${entry.name}: ${e instanceof Error ? e.message : String(e)}`);
    }
    if (count % 1000 === 0) console.error(`[decode-tile] ${count} pages read, ${bad.length} at fault`);
  }
  for (const line of bad) console.error(`  ${line}`);
  if (bad.length > 0) {
    console.error(`[decode-tile] ${bad.length} of ${count} page(s) under ${pages} at fault`);
    Deno.exit(1);
  }
  if (count === 0) {
    console.error(`[decode-tile] no pages under ${pages} — bake first, or name the version`);
    Deno.exit(1);
  }
  console.log(`[decode-tile] ok — ${count} page(s) under ${pages} read as the kernel reads them`);
} else {
  if (target === undefined) {
    console.error("usage: decode-tile.ts <key | path> [--version v3] [--out dir] | --all [--version v3]");
    Deno.exit(2);
  }
  const path = /^[0-9]+$/.test(target) ? join(pages, target) : target;
  const page = decode(await readPage(path));
  const s = summarize(page);
  console.log(JSON.stringify({ page: basename(path), ...s }, null, 2));
  const f = faults(s);
  for (const line of f) console.error(`[decode-tile] ${line}`);

  const dir = outDir ?? join(root, "out/decoded", basename(path));
  await Deno.mkdir(dir, { recursive: true });
  await Deno.writeFile(join(dir, "heights.png"), await heightsPng(page.ele, s));
  for (const ch of CHANNELS) await Deno.writeFile(join(dir, `${ch}.png`), await planePng(ch, page.planes[ch]));
  console.log(`[decode-tile] wrote heights.png and ${CHANNELS.map((c) => `${c}.png`).join(", ")} under ${dir}`);
  if (f.length > 0) Deno.exit(1);
}
