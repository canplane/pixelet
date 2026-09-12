// @pxl/web/modules/world/billboard/atlas.ts

/**
 * Every billboard's bitmap in one texture.
 *
 * What this replaces gave each badge its own CanvasTexture. That is one GPU
 * allocation and one material binding per badge, which is one draw call per
 * badge, because two sprites with different textures cannot be batched.
 * Holding them in one image is what makes a single instanced draw possible
 * at all — the atlas is not the optimisation, it is the precondition for it.
 *
 * It is worth being clear about what it does *not* buy: the same badges at
 * the same resolution occupy the same pixels whether they live in one image
 * or five hundred. Memory comes down by drawing smaller or keeping fewer,
 * not by packing.
 *
 * **Shelf packing, because the height is nearly fixed.** Badges are a line
 * of text: width varies with the name, height does not. Rows of a bucketed
 * height, filled left to right, put the general rectangle-packing problem
 * out of scope entirely — and bucketing the width as well makes a freed slot
 * reusable by the next badge of roughly that size, which is what keeps a
 * long session from fragmenting the sheet into unusable gaps.
 *
 * **Growth repacks rather than reserving.** A sheet big enough for a dense
 * city is most of a hundred megabytes, and most sessions never need it, so
 * this starts small and doubles. Each entry keeps the closure that drew it,
 * so a repack is a redraw — which costs what building the badges cost in the
 * first place, a few tens of milliseconds, a handful of times in a session.
 * The alternative is asking every owner to re-register, which spreads one
 * concern across everything that ever drew into it.
 *
 * ---
 *
 * **What would replace this: an atlas of glyphs rather than of badges.**
 *
 * The ceiling here is set by the data. Every distinct place name is a
 * distinct rectangle, so a dense city fills the sheet and there is no size
 * that is enough — MAX_SIZE is a place to give up, not a bound. Pack
 * *glyphs* instead and the sheet is bounded by the script: a few hundred
 * Latin forms and however many Hangul syllables are on screen, shared by
 * every badge that uses them. That is what map renderers do, and it is the
 * only version of this that stops growing with the world.
 *
 * It costs a badge becoming a run of quads rather than one, so the layer
 * that draws them has to lay out text — which is why it belongs *after*
 * instancing and not before. Once a badge is already an instanced quad with
 * its rectangle in a vertex attribute, a badge made of eight glyph quads is
 * the same machinery with a different count. Doing it first means writing
 * text layout against the sprite path and then again against the instanced
 * one.
 *
 * It also changes what a badge costs to build. Today that is a canvas draw
 * and an upload; then it would be a measure and a list of quads, with the
 * upload happening once per glyph ever seen. The 0.09 ms per badge that
 * made entering a dense area expensive is mostly rasterisation, and this is
 * where it goes.
 */

import * as THREE from "three";

import type { f64, u32 } from "@xpute/core/kit/word.ts";

import { make_canvas_texture } from "./canvas_texture.ts";

/**
 * Where an entry sits, in texture coordinates, in the canvas's own
 * orientation: v0 is the top edge and v1 the bottom, so v0 < v1.
 *
 * The sheet is uploaded with `flipY` off, which is the whole reason these
 * read naturally. A texture left at the default is flipped on upload, which
 * a single-image sprite never notices — its UVs span the lot either way —
 * and an atlas notices immediately, because a rectangle measured from the
 * top then addresses a band measured from the bottom. Consumers map v1 to
 * the bottom of whatever they draw; for a sprite that means a negative
 * repeat, which is the flip expressed once, where it can be seen.
 */
export type AtlasUV = { u0: f64; v0: f64; u1: f64; v1: f64 };

export type AtlasStats = {
  /** Sheet edge in device pixels; the sheet is square. */
  size: u32;
  entries: u32;
  /** Rows opened, and how much of their length is spent. */
  shelves: u32;
  used_px: u32;
  repacks: u32;
};

/** Freed slots are reused by size class rather than by exact fit, so a name
 * one pixel narrower than the last one still lands in the hole it left.
 * Sixteen device pixels is eight logical ones at the render scale badges
 * are drawn at — below noticing on a name, and coarse enough that the
 * classes stay few. */
const WIDTH_BUCKET = 16;
const HEIGHT_BUCKET = 16;

const INITIAL_SIZE = 1024;
const MAX_SIZE = 4096;
/** One device pixel of transparent margin, so a linear sample at the edge of
 * one entry cannot reach into its neighbour. Mipmaps make the reach wider
 * than a texel at distance, which is exactly when badges are small and their
 * neighbours are close by in the sheet. */
const PAD = 1;

type Entry = {
  x: u32;
  y: u32;
  /** The slot, bucketed — what the shelf spends and the free list keys on. */
  w: u32;
  h: u32;
  /** What was actually drawn, which is smaller: bucketing rounds the slot up
   * so a freed one is reusable, and the difference is transparent padding
   * that no UV may include. Spanning the slot instead squeezed every badge
   * horizontally by up to a bucket. */
  cw: u32;
  ch: u32;
  uv: AtlasUV;
  /** Kept so a repack can replay it. The context it is handed is already
   * translated to the entry's own origin. */
  draw: (ctx: CanvasRenderingContext2D) => void;
  live: boolean;
};

type Shelf = {
  y: u32;
  /** Bucketed, and every entry in the row is allocated at this height. */
  h: u32;
  /** How far along the row is spent. */
  cursor: u32;
};

const bucket = (v: f64, to: u32): u32 => Math.ceil(v / to) * to;

/** Where every entry would go on a sheet of `size`, or null if one does not
 * fit. The same first-fit shelf rule `place` uses, run on scratch shelves so
 * a failed plan changes nothing. */
type Plan = { xs: u32[]; ys: u32[]; shelves: Shelf[]; next_y: u32 };

function pack(size: u32, entries: readonly { w: u32; h: u32 }[]): Plan | null {
  const shelves: Shelf[] = [];
  let next_y: u32 = 0;
  const xs: u32[] = [], ys: u32[] = [];
  for (const { w, h } of entries) {
    let shelf = shelves.find((s) => s.h === h && s.cursor + w <= size);
    if (!shelf) {
      if (next_y + h > size || w > size) return null;
      shelf = { y: next_y, h, cursor: 0 };
      next_y += h;
      shelves.push(shelf);
    }
    xs.push(shelf.cursor);
    ys.push(shelf.y);
    shelf.cursor += w;
  }
  return { xs, ys, shelves, next_y };
}

export class BillboardAtlas {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private size: u32 = INITIAL_SIZE;

  texture: THREE.CanvasTexture;

  private shelves: Shelf[] = [];
  private next_y: u32 = 0;
  /** Freed slots by `${h}:${w}` size class, for reuse before the cursor
   * moves. A freed entry's pixels stay on the sheet until something else
   * draws over them, which nothing samples because nothing points there. */
  private free: Map<string, Entry[]> = new Map();
  private entries = new Set<Entry>();

  private used_px: u32 = 0;
  private repacks: u32 = 0;

  constructor() {
    this.canvas = document.createElement("canvas");
    this.canvas.width = this.size;
    this.canvas.height = this.size;
    this.ctx = this.canvas.getContext("2d")!;
    this.texture = this.make_sheet_texture();
  }

  private make_sheet_texture(): THREE.CanvasTexture {
    const t = make_canvas_texture(this.canvas);
    // See AtlasUV: an atlas cannot use the upload flip, because a rectangle
    // measured from the top would address a band measured from the bottom.
    t.flipY = false;
    return t;
  }

  /**
   * Draws into the sheet and returns where it landed, or null when the sheet
   * is at its limit and nothing that size is free.
   *
   * `w`/`h` are device pixels — the caller's logical size times whatever
   * supersampling it draws at.
   */
  add(w: f64, h: f64, draw: (ctx: CanvasRenderingContext2D) => void): AtlasUV | null {
    const bw = bucket(w + PAD * 2, WIDTH_BUCKET);
    const bh = bucket(h + PAD * 2, HEIGHT_BUCKET);

    let entry = this.take_free(bw, bh);
    if (!entry) entry = this.place(bw, bh);
    while (!entry) {
      if (!this.grow()) return null;
      entry = this.place(bw, bh);
    }

    entry.cw = Math.ceil(w);
    entry.ch = Math.ceil(h);
    this.set_uv(entry);
    entry.draw = draw;
    entry.live = true;
    this.entries.add(entry);
    this.paint(entry);
    this.texture.needsUpdate = true;
    return entry.uv;
  }

  /** Returns an entry's space to the pool. The pixels are left alone: the
   * only thing that made them visible was a UV pointing at them. */
  release(uv: AtlasUV): void {
    for (const entry of this.entries) {
      if (entry.uv !== uv) continue;
      this.entries.delete(entry);
      entry.live = false;
      this.used_px -= entry.w * entry.h;
      const key = `${entry.h}:${entry.w}`;
      const list = this.free.get(key) ?? [];
      list.push(entry);
      this.free.set(key, list);
      return;
    }
  }

  stats(): AtlasStats {
    return { size: this.size, entries: this.entries.size, shelves: this.shelves.length, used_px: this.used_px, repacks: this.repacks };
  }

  private take_free(w: u32, h: u32): Entry | null {
    const list = this.free.get(`${h}:${w}`);
    const entry = list?.pop();
    if (!entry) return null;
    this.used_px += entry.w * entry.h;
    return entry;
  }

  private place(w: u32, h: u32): Entry | null {
    let shelf = this.shelves.find((s) => s.h === h && s.cursor + w <= this.size);
    if (!shelf) {
      if (this.next_y + h > this.size) return null;
      shelf = { y: this.next_y, h, cursor: 0 };
      this.next_y += h;
      this.shelves.push(shelf);
    }

    const x = shelf.cursor;
    shelf.cursor += w;
    this.used_px += w * h;
    return {
      x,
      y: shelf.y,
      w,
      h,
      cw: w - PAD * 2,
      ch: h - PAD * 2,
      uv: { u0: 0, v0: 0, u1: 0, v1: 0 },
      draw: () => {},
      live: true,
    };
  }

  /** The window over what was drawn, not over the slot it was drawn in.
   * Written in place: whoever holds the object keeps holding the right
   * rectangle across a repack. */
  private set_uv(entry: Entry): void {
    entry.uv.u0 = (entry.x + PAD) / this.size;
    entry.uv.v0 = (entry.y + PAD) / this.size;
    entry.uv.u1 = (entry.x + PAD + entry.cw) / this.size;
    entry.uv.v1 = (entry.y + PAD + entry.ch) / this.size;
  }

  private paint(entry: Entry): void {
    const ctx = this.ctx;
    ctx.save();
    ctx.clearRect(entry.x, entry.y, entry.w, entry.h);
    ctx.beginPath();
    ctx.rect(entry.x + PAD, entry.y + PAD, entry.w - PAD * 2, entry.h - PAD * 2);
    ctx.clip();
    ctx.translate(entry.x + PAD, entry.y + PAD);
    entry.draw(ctx);
    ctx.restore();
  }

  /**
   * Halves the sheet for as long as every live entry still fits in half.
   *
   * Growth only ever went one way, so a sheet that doubled to 4096 crossing
   * the densest part of the map stayed at 4096 after the badges that needed
   * it were gone — the ledger read 12 entries on a 4096 sheet, which is a
   * 64 MB canvas and another 64 MB of texture holding almost nothing.
   *
   * Planned before anything moves: the entries are laid out on the smaller
   * sheet first, and only if every one of them lands is the sheet rebuilt. A
   * cheap area test goes first so a sheet that is still mostly in use is not
   * even tried. Called when badges are evicted, which is the only time the
   * live set shrinks.
   */
  compact(): boolean {
    let changed = false;
    while (this.size > INITIAL_SIZE) {
      const target = this.size / 2;
      if (this.used_px > (target * target) / 2) break;
      const live = [...this.entries];
      const plan = pack(target, live);
      if (!plan) break;
      this.rebuild(target, live, plan);
      changed = true;
    }
    return changed;
  }

  /**
   * Doubles the sheet and replays every live entry onto it.
   *
   * The UV objects are mutated in place rather than replaced, so whatever
   * holds one keeps holding the right rectangle without being told. That is
   * the reason `add` hands back the object it stores instead of a copy.
   */
  private grow(): boolean {
    if (this.size >= MAX_SIZE) return false;
    const live = [...this.entries];
    const plan = pack(this.size * 2, live);
    // The sheet doubles and held all of these at half the size, so there is
    // room by construction; a null here would mean the packer disagrees
    // with itself.
    if (!plan) return false;
    this.rebuild(this.size * 2, live, plan);
    return true;
  }

  /**
   * Replaces the sheet with one of `size` and repaints `live` at the places
   * `plan` gave them. The one path both growth and compaction take.
   */
  private rebuild(size: u32, live: Entry[], plan: Plan): void {
    this.size = size;
    // A new canvas, not a resize of the live one. three allocates immutable
    // storage for a texture once (texStorage2D, WebGL2) and every later
    // upload is a texSubImage2D into it, so a canvas that changes size under
    // a texture that has already uploaded overflows what was allocated —
    // GL_INVALID_VALUE from glCopySubTextureCHROMIUM, silent on the page and
    // fatal to the sheet. Replacing the element means no texture ever sees
    // its own image change size.
    this.canvas = document.createElement("canvas");
    this.canvas.width = size;
    this.canvas.height = size;
    this.ctx = this.canvas.getContext("2d")!;

    this.shelves = plan.shelves;
    this.next_y = plan.next_y;
    // Freed slots belonged to the old layout; the plan packed only live
    // entries, so nothing is free on the new sheet but unshelved space.
    this.free.clear();
    this.used_px = 0;
    this.repacks++;
    for (let i = 0; i < live.length; i++) {
      const entry = live[i];
      entry.x = plan.xs[i];
      entry.y = plan.ys[i];
      // Same object, new rectangle — see grow's own doc comment.
      this.set_uv(entry);
      this.used_px += entry.w * entry.h;
      this.paint(entry);
    }

    // A resized canvas is a different image as far as the GPU is concerned.
    this.texture.dispose();
    this.texture = this.make_sheet_texture();
  }
}
