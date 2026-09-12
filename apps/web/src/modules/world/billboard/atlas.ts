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

import type { f64, i32, u32 } from "@xpute/core/kit/word.ts";
import { Vector } from "@xpute/core/ds/arena.ts";

import { CORE_ALLOC } from "@/render/layer/pipeline/core_pool.ts";

import { make_canvas_texture } from "./canvas_texture.ts";

/**
 * An entry is a slot, named by its index, and a consumer keeps that index
 * for as long as the badge is cached. Its rectangle is read through the
 * `u0`..`v1` accessors, in texture coordinates in the canvas's own
 * orientation: v0 is the top edge and v1 the bottom, so v0 < v1.
 *
 * The sheet is uploaded with `flipY` off, which is the whole reason these
 * read naturally. A texture left at the default is flipped on upload, which
 * a single-image sprite never notices — its UVs span the lot either way —
 * and an atlas notices immediately, because a rectangle measured from the
 * top then addresses a band measured from the bottom. Consumers map v1 to
 * the bottom of whatever they draw; for a sprite that means a negative
 * repeat, which is the flip expressed once, where it can be seen.
 *
 * Read on every use rather than once: the rectangle moves when the sheet
 * repacks, and the slot index is what stays.
 */
export const NO_SLOT: i32 = -1;

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

/** A slot's state. UNUSED never held anything or lost its rectangle to a
 * repack; FREE was released and its rectangle can be taken by an entry of
 * the same size class; LIVE is drawn. */
const UNUSED = 0;
const FREE = 1;
const LIVE = 2;

const LANE = { init_cap: 256, alloc: CORE_ALLOC } as const;

const bucket = (v: f64, to: u32): u32 => Math.ceil(v / to) * to;

/**
 * Where every entry would go on a sheet of `size`, or null if one does not
 * fit — the same first-fit shelf rule `place` uses, run on scratch shelves
 * so a failed plan changes nothing. `ws`/`hs` are the entries' bucketed
 * sizes in the order they are to be placed.
 */
type Plan = { xs: Uint32Array; ys: Uint32Array; shelf_y: u32[]; shelf_h: u32[]; shelf_cursor: u32[]; next_y: u32 };

function pack(size: u32, ws: Uint32Array, hs: Uint32Array, n: u32): Plan | null {
  const shelf_y: u32[] = [], shelf_h: u32[] = [], shelf_cursor: u32[] = [];
  let next_y: u32 = 0;
  const xs = new Uint32Array(n), ys = new Uint32Array(n);
  for (let i = 0; i < n; i++) {
    const w = ws[i], h = hs[i];
    let shelf = -1;
    for (let s = 0; s < shelf_h.length; s++) {
      if (shelf_h[s] === h && shelf_cursor[s] + w <= size) {
        shelf = s;
        break;
      }
    }
    if (shelf < 0) {
      if (next_y + h > size || w > size) return null;
      shelf = shelf_h.length;
      shelf_y.push(next_y);
      shelf_h.push(h);
      shelf_cursor.push(0);
      next_y += h;
    }
    xs[i] = shelf_cursor[shelf];
    ys[i] = shelf_y[shelf];
    shelf_cursor[shelf] += w;
  }
  return { xs, ys, shelf_y, shelf_h, shelf_cursor, next_y };
}

export class BillboardAtlas {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private size: u32 = INITIAL_SIZE;

  texture: THREE.CanvasTexture;

  /** The rows: where each starts, its bucketed height, how far along it is
   * spent. Every entry in a row is allocated at the row's height. */
  private readonly shelf_y = new Vector(Uint32Array, LANE);
  private readonly shelf_h = new Vector(Uint32Array, LANE);
  private readonly shelf_cursor = new Vector(Uint32Array, LANE);
  private next_y: u32 = 0;

  /**
   * The entries, one slot each. `w`/`h` is the slot, bucketed — what the
   * shelf spends and what a FREE one is matched on. `cw`/`ch` is what was
   * actually drawn, which is smaller: bucketing rounds the slot up so a
   * freed one is reusable, and the difference is transparent padding that
   * no UV may include. Spanning the slot instead squeezed every badge
   * horizontally by up to a bucket.
   */
  private readonly state = new Vector(Uint8Array, LANE);
  private readonly x = new Vector(Uint32Array, LANE);
  private readonly y = new Vector(Uint32Array, LANE);
  private readonly w = new Vector(Uint32Array, LANE);
  private readonly h = new Vector(Uint32Array, LANE);
  private readonly cw = new Vector(Uint32Array, LANE);
  private readonly ch = new Vector(Uint32Array, LANE);
  /** What drew each entry, kept so a repack can replay it. The context it
   * is handed is already translated to the entry's own origin. */
  private readonly draws: ((ctx: CanvasRenderingContext2D) => void)[] = [];

  private live: u32 = 0;
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
    // See the header: an atlas cannot use the upload flip, because a
    // rectangle measured from the top would address a band measured from
    // the bottom.
    t.flipY = false;
    return t;
  }

  // ---- the rectangle of a slot ----

  u0(slot: u32): f64 {
    return (this.x.mem[slot] + PAD) / this.size;
  }
  v0(slot: u32): f64 {
    return (this.y.mem[slot] + PAD) / this.size;
  }
  u1(slot: u32): f64 {
    return (this.x.mem[slot] + PAD + this.cw.mem[slot]) / this.size;
  }
  v1(slot: u32): f64 {
    return (this.y.mem[slot] + PAD + this.ch.mem[slot]) / this.size;
  }

  /**
   * Draws into the sheet and returns the slot it landed in, or NO_SLOT when
   * the sheet is at its limit and nothing that size is free.
   *
   * `w`/`h` are device pixels — the caller's logical size times whatever
   * supersampling it draws at.
   */
  add(w: f64, h: f64, draw: (ctx: CanvasRenderingContext2D) => void): i32 {
    const bw = bucket(w + PAD * 2, WIDTH_BUCKET);
    const bh = bucket(h + PAD * 2, HEIGHT_BUCKET);

    let slot = this.take_free(bw, bh);
    if (slot < 0) slot = this.place(bw, bh);
    while (slot < 0) {
      if (!this.grow()) return NO_SLOT;
      slot = this.place(bw, bh);
    }

    this.cw.mem[slot] = Math.ceil(w);
    this.ch.mem[slot] = Math.ceil(h);
    this.draws[slot] = draw;
    this.state.mem[slot] = LIVE;
    this.live++;
    this.paint(slot);
    this.texture.needsUpdate = true;
    return slot;
  }

  /** Returns a slot's space to the pool. The pixels are left alone: the
   * only thing that made them visible was a UV pointing at them. */
  release(slot: u32): void {
    if (this.state.mem[slot] !== LIVE) return;
    this.state.mem[slot] = FREE;
    this.live--;
    this.used_px -= this.w.mem[slot] * this.h.mem[slot];
  }

  stats(): AtlasStats {
    return { size: this.size, entries: this.live, shelves: this.shelf_h.len, used_px: this.used_px, repacks: this.repacks };
  }

  /** A released slot of this size class, taken back. */
  private take_free(w: u32, h: u32): i32 {
    const state = this.state.mem;
    for (let slot = 0; slot < this.state.len; slot++) {
      if (state[slot] !== FREE || this.w.mem[slot] !== w || this.h.mem[slot] !== h) continue;
      this.used_px += w * h;
      return slot;
    }
    return NO_SLOT;
  }

  /** A slot index nobody holds, reused before a new one is opened. */
  private unused_slot(): u32 {
    const state = this.state.mem;
    for (let slot = 0; slot < this.state.len; slot++) if (state[slot] === UNUSED) return slot;
    const slot = this.state.len;
    this.state.push(UNUSED);
    this.x.push(0);
    this.y.push(0);
    this.w.push(0);
    this.h.push(0);
    this.cw.push(0);
    this.ch.push(0);
    return slot;
  }

  private place(w: u32, h: u32): i32 {
    let shelf = -1;
    for (let s = 0; s < this.shelf_h.len; s++) {
      if (this.shelf_h.mem[s] === h && this.shelf_cursor.mem[s] + w <= this.size) {
        shelf = s;
        break;
      }
    }
    if (shelf < 0) {
      if (this.next_y + h > this.size) return NO_SLOT;
      shelf = this.shelf_h.len;
      this.shelf_y.push(this.next_y);
      this.shelf_h.push(h);
      this.shelf_cursor.push(0);
      this.next_y += h;
    }

    const slot = this.unused_slot();
    this.x.mem[slot] = this.shelf_cursor.mem[shelf];
    this.y.mem[slot] = this.shelf_y.mem[shelf];
    this.w.mem[slot] = w;
    this.h.mem[slot] = h;
    this.shelf_cursor.mem[shelf] += w;
    this.used_px += w * h;
    return slot;
  }

  private paint(slot: u32): void {
    const ctx = this.ctx;
    const x = this.x.mem[slot], y = this.y.mem[slot], w = this.w.mem[slot], h = this.h.mem[slot];
    ctx.save();
    ctx.clearRect(x, y, w, h);
    ctx.beginPath();
    ctx.rect(x + PAD, y + PAD, w - PAD * 2, h - PAD * 2);
    ctx.clip();
    ctx.translate(x + PAD, y + PAD);
    this.draws[slot](ctx);
    ctx.restore();
  }

  /** The live slots, in index order, and their bucketed sizes — what a
   * repack lays out. */
  private live_slots(): { slots: Uint32Array; ws: Uint32Array; hs: Uint32Array } {
    const slots = new Uint32Array(this.live), ws = new Uint32Array(this.live), hs = new Uint32Array(this.live);
    let n = 0;
    for (let slot = 0; slot < this.state.len; slot++) {
      if (this.state.mem[slot] !== LIVE) continue;
      slots[n] = slot;
      ws[n] = this.w.mem[slot];
      hs[n] = this.h.mem[slot];
      n++;
    }
    return { slots, ws, hs };
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
      const live = this.live_slots();
      const plan = pack(target, live.ws, live.hs, this.live);
      if (!plan) break;
      this.rebuild(target, live.slots, plan);
      changed = true;
    }
    return changed;
  }

  /** Doubles the sheet and replays every live entry onto it. The slots keep
   * their indices; only their rectangles move, which is why a consumer
   * reads the rectangle on every use. */
  private grow(): boolean {
    if (this.size >= MAX_SIZE) return false;
    const live = this.live_slots();
    const plan = pack(this.size * 2, live.ws, live.hs, this.live);
    // The sheet doubles and held all of these at half the size, so there is
    // room by construction; a null here would mean the packer disagrees
    // with itself.
    if (!plan) return false;
    this.rebuild(this.size * 2, live.slots, plan);
    return true;
  }

  /**
   * Replaces the sheet with one of `size` and repaints the live slots at
   * the places `plan` gave them. The one path both growth and compaction
   * take.
   */
  private rebuild(size: u32, slots: Uint32Array, plan: Plan): void {
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

    this.shelf_y.truncate(0);
    this.shelf_h.truncate(0);
    this.shelf_cursor.truncate(0);
    for (let s = 0; s < plan.shelf_h.length; s++) {
      this.shelf_y.push(plan.shelf_y[s]);
      this.shelf_h.push(plan.shelf_h[s]);
      this.shelf_cursor.push(plan.shelf_cursor[s]);
    }
    this.next_y = plan.next_y;
    // Freed slots belonged to the old layout; the plan packed only live
    // entries, so nothing is free on the new sheet but unshelved space.
    for (let slot = 0; slot < this.state.len; slot++) if (this.state.mem[slot] === FREE) this.state.mem[slot] = UNUSED;
    this.used_px = 0;
    this.repacks++;
    for (let i = 0; i < slots.length; i++) {
      const slot = slots[i];
      this.x.mem[slot] = plan.xs[i];
      this.y.mem[slot] = plan.ys[i];
      this.used_px += this.w.mem[slot] * this.h.mem[slot];
      this.paint(slot);
    }

    // A resized canvas is a different image as far as the GPU is concerned.
    this.texture.dispose();
    this.texture = this.make_sheet_texture();
  }
}
