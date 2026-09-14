// @pxl/web/wasm/core_build.ts

/**
 * The glue's half of the chunks' scheduling and builds, which run in the
 * module (crates/pxl/core/src/{sched,build}.rs): commands into the
 * submission ring, the frame written where the module reads it, one
 * `pxl_tick` a frame, and the completion ring read when it returns.
 *
 * A chunk here is an id and a sink. The node says what the chunk is made of;
 * the module decides whether it is on screen, which tiers it shows and how a
 * blend weighs them, builds the tiers under the frame's budget, and tells the
 * sink — a mesh handed over whole in a block of the mesh section the sink
 * then owns, a layout to show, a tier to let go. A command is applied at the
 * next tick, or at once when the ring is full — `pump` makes room — so a
 * sequence of them is applied in the order it was sent.
 *
 * The TypeScript scan and mesher stay as the references the module is held
 * to (wasm_bake.test.ts, wasm_scan.test.ts); nothing on screen is built by
 * them.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import { Errno } from "@xpute/core/sys/errno.ts";
import type { U8Array } from "@xpute/core/kit/array.ts";
import { FrameFlag } from "@xpute/core/ipc/frame.ts";
import { Ring } from "@xpute/core/ipc/ring.ts";
import { type BranchCursor, type BranchView, encoder, TreeReader, TreeView } from "@xpute/core/io/marshal/tree/mod.ts";
import { Cmd } from "@pxl/shared/spec/cmd.ts";
import { GROUND_CH_KEYS, type Terrain } from "@pxl/shared/bake/format.ts";

import type { LodStep } from "@/module/world/lod.ts";
import type { PxlExports } from "@/wasm/pxl.ts";
import type { MeshBlock } from "@/render/mesh/mesh_block.ts";
import { decor_block_carve, decor_block_release, type DecorBlock } from "@/render/mesh/decor_block.ts";
import { CORE_POOL, CORE_PXL, FIXED_ALLOC } from "@/wasm/core_pool.ts";

/** Hands one baked mesh's block back to the core pool. Every view into it is
 * dead afterwards. */
export function release_chunk_mesh(mesh: { block: MeshBlock["block"] }): void {
  CORE_POOL.release(mesh.block);
}

/** crates/pxl/wasm/src/lib.rs's frame block: doubles, in this order. */
export const FRAME_WORDS = 62;
export const FRAME = {
  root: 0,
  origin: 16,
  planes: 19,
  camera: 43,
  lead: 46,
  viewport_h: 49,
  fov_deg: 50,
  delta_s: 51,
  budget_ms: 52,
  hard_slack_ms: 53,
  pixel_dither: 54,
  retain_budget_bytes: 55,
  anchor_x: 56,
  anchor_y: 57,
  has_anchor: 58,
  render_dist: 59,
  range_hysteresis: 60,
  show_decor: 61,
} as const;

/** What the stage is told about the chunks the module mounts from the range. */
export type MountListener = {
  /** A chunk came into range: its page and page-local base, in units. The
   * listener makes its node and attaches it as the chunk's sink. */
  on_mount(id: u32, page_idx: bigint, base_x: u32, base_y: u32): void;
  /** A chunk left the range, after its sink heard on_unmount. */
  on_unmount(id: u32): void;
};

/** What a chunk is told. The module draws from its own lists; what the
 * sink holds is the blocks, for as long as the module names them. */
export type ChunkBuildSink = {
  /** Tier `step`, built from the sources of `generation`; the block is the
   * sink's to keep until the module evicts the step, or to release at once
   * when it is stale. */
  on_tier(generation: u32, step: LodStep, mesh: MeshBlock): void;
  /** The bytes the chunk's runs hold now. */
  on_runs(bytes: u32): void;
  /** Whether the chunk is on screen: what the module decided, told for a
   * reader that wants it (the reference tests); the draws follow it. */
  on_visible?(on: boolean): void;
  /** The layout the module shows, whose tiers the sink holds; not ready is
   * nothing. */
  on_layout(primary: LodStep, secondary: LodStep | null, ready: boolean): void;
  /** The layout's blend, as `on_visible`: the draws carry it. */
  on_blend?(primary: number, secondary: number, decor_secondary: boolean): void;
  /** Let go of a tier and the decor at its step: the module no longer
   * names them. */
  on_evict(step: LodStep): void;
  /** The chunk's decor at `step`, the sink's to keep or release; a layout
   * comes after the decor of every tier in it. */
  on_decor(step: LodStep, decor: DecorBlock): void;
  /** The module unmounted the chunk: nothing more comes. */
  on_unmount?(): void;
};

/** The scheduler's census, for the ledger. */
export type Census = {
  chunks: u32;
  in_frustum: u32;
  settling: u32;
  blend_active: u32;
  pending: u32;
  offscreen_pending: u32;
  oldest_visible_ms: number;
  oldest_release_ms: number;
  committed: u32;
  released: u32;
  retained_on: number;
  retained_off: number;
  /** How far the byte budget has pulled the LOD ladder in (sched.rs). */
  pressure: u32;
};

/** The build counters the ledger reads, cumulative since load. */
export type BuildStat = {
  direct: u32;
  scans: u32;
  bakes: u32;
  starved: u32;
  total_runs: u32;
  culled: u32;
  faces_possible: u32;
  faces_kept: u32;
  coarse_total: u32;
  coarse_low_fill: u32;
  /** The module heap's use, not cumulative. */
  heap_used: u32;
};

/** A packet of one branch, its children written by `fill` — what a message
 * carries (@pxl/shared/spec/cmd.ts). */
export function packet(fill: (b: BranchView) => void): U8Array {
  return encoder.encode(new TreeView().branch(fill));
}

export class CoreBuild {
  private readonly sq: Ring;
  private readonly cq: Ring;
  private readonly sinks = new Map<u32, ChunkBuildSink>();
  private next_id: u32 = 1;
  /** The call waiting on its reply: its tag, and what reads the reply. */
  private next_call: u32 = 0;
  private reply_tag: u32 = 0;
  private reply: ((result: number, f: BranchCursor | null) => void) | null = null;
  /** Who makes and drops the nodes of mounted chunks. */
  mounts: MountListener | null = null;
  private order: Uint32Array;
  /** Where the frame is written before a tick. */
  readonly frame: Float64Array;
  private warned_unreachable = false;

  constructor(private readonly pxl: PxlExports) {
    this.sq = new Ring(pxl.memory.buffer, pxl.pxl_ring_sq());
    this.cq = new Ring(pxl.memory.buffer, pxl.pxl_ring_cq());
    this.order = FIXED_ALLOC.take(Uint32Array, 1024);
    this.frame = new Float64Array(pxl.memory.buffer, pxl.pxl_frame_at(), FRAME_WORDS);
    this.census_row = new Float64Array(pxl.memory.buffer, pxl.pxl_census(), 13);
    this.stat_row = new Uint32Array(pxl.memory.buffer, pxl.pxl_build_stat(), 11);
  }

  /** Makes room in the submission ring: applies what waits, and hands out
   * what that made. */
  pump(): void {
    this.pxl.pxl_drain();
    this.read();
  }

  private submit(tag: u32, cmd: Cmd, pkt: U8Array | null = null, flags: u32 = 0): void {
    for (;;) {
      const errno = this.sq.push(tag, cmd, flags, pkt);
      if (errno === Errno.OK) return;
      if (errno !== Errno.EAGAIN) throw new Error(`[wasm/core_build] command 0x${cmd.toString(16)} refused by the ring: ${errno}`);
      this.pump();
    }
  }

  private read(): void {
    const cq = this.cq;
    for (let at = cq.peek(); at >= 0; at = cq.peek()) {
      const id = cq.tag(at);
      const cmd = cq.cmd(at) as Cmd;
      const pkt = cq.packet(at);
      const f: BranchCursor | null = pkt ? new TreeReader(pkt).read_branch() : null;
      if (cq.flags(at) & FrameFlag.RES) {
        if (this.reply !== null && id === this.reply_tag) {
          this.reply(cq.result(at), f);
          this.reply = null;
        } else if (cq.result(at) < 0) {
          console.warn(`[wasm/core_build] command 0x${cmd.toString(16)} failed: errno ${cq.result(at)}`);
        }
        cq.advance();
        continue;
      }
      const u32_at = (i: u32) => f!.at(i).get<number>();
      if (cmd === Cmd.CHUNK_CREATE) {
        this.mounts?.on_mount(id, f!.at(0).get<bigint>(), u32_at(1), u32_at(2));
        cq.advance();
        continue;
      }
      if (cmd === Cmd.CHUNK_DELETE) {
        this.sinks.get(id)?.on_unmount?.();
        this.sinks.delete(id);
        this.mounts?.on_unmount(id);
        cq.advance();
        continue;
      }
      const sink = this.sinks.get(id);
      if (cmd === Cmd.TIER_CREATE) {
        const mesh: MeshBlock = { block: CORE_POOL.adopt(u32_at(2), u32_at(3)), vertex_cnt: u32_at(4), index_cnt: u32_at(5) };
        if (sink) sink.on_tier(u32_at(0), u32_at(1) as LodStep, mesh);
        else release_chunk_mesh(mesh);
      } else if (cmd === Cmd.DECOR_CREATE) {
        const decor = decor_block_carve(CORE_POOL.adopt(u32_at(1), u32_at(2)));
        if (sink) sink.on_decor(u32_at(0) as LodStep, decor);
        else decor_block_release(decor);
      } else if (sink) {
        if (cmd === Cmd.RUNS_UPDATE) sink.on_runs(u32_at(0));
        else if (cmd === Cmd.CHUNK_VISIBLE) sink.on_visible?.(f!.at(0).get<boolean>());
        else if (cmd === Cmd.CHUNK_LAYOUT) sink.on_layout(u32_at(0) as LodStep, u32_at(1) === 0 ? null : u32_at(1) as LodStep, f!.at(2).get<boolean>());
        else if (cmd === Cmd.CHUNK_BLEND) sink.on_blend?.(u32_at(0), u32_at(1), f!.at(2).get<boolean>());
        else if (cmd === Cmd.TIER_DELETE) sink.on_evict(u32_at(0) as LodStep);
      }
      cq.advance();
    }
  }

  /** Sends a command that asks for no reply. It is applied at the module's
   * next drain, tick or read, in the order it was sent. */
  send(tag: u32, cmd: Cmd, pkt: U8Array | null = null): void {
    this.submit(tag, cmd, pkt);
  }

  /** Applies every command sent so far, now, without handing out what they
   * made: for a caller about to read what it changed, and safe inside a
   * sink, since no sink runs. */
  settle(): void {
    this.pxl.pxl_drain();
  }

  /**
   * A query answered in the turn it is asked: submitted with ACKREQ behind
   * whatever waits — so a placement just sent is what it sees — and the
   * kernel rung until the reply comes back, the signals before it handed
   * out as `read` always does. `take` reads the reply; its packet is valid
   * only inside it.
   */
  call<T>(cmd: Cmd, pkt: U8Array | null, take: (result: number, reply: BranchCursor | null) => T): T {
    const tag = this.next_call = (this.next_call + 1) >>> 0;
    let out: { value: T } | null = null;
    this.reply_tag = tag;
    this.reply = (result, f) => out = { value: take(result, f) };
    try {
      this.submit(tag, cmd, pkt, FrameFlag.ACKREQ);
      while (out === null) {
        const posted = this.pxl.pxl_drain();
        this.read();
        if (out === null && posted === 0 && this.sq.len === 0) throw new Error(`[wasm/core_build] command 0x${cmd.toString(16)} was never replied to`);
      }
    } finally {
      this.reply = null;
    }
    return (out as { value: T }).value;
  }

  /** A new chunk whose column's base is `base` — its X and Y in units
   * beside it — starting at page cell (tx0, ty0), with the hash that picks
   * its side of a per-chunk blend: its id. */
  add(sink: ChunkBuildSink, base: bigint, base_x: number, base_y: number, tx0: u32, ty0: u32, stable_hash: u32 = 0): u32 {
    const id = this.next_id++;
    this.sinks.set(id, sink);
    const b = BigInt.asUintN(64, base);
    this.submit(id, Cmd.CHUNK_CREATE, packet((p) => p.u64(b).u32(tx0).u32(ty0).i32(base_x | 0).i32(base_y | 0).u32(stable_hash >>> 0)));
    return id;
  }

  /** Makes `sink` the one a mounted chunk's events reach. */
  attach(id: u32, sink: ChunkBuildSink): void {
    this.sinks.set(id, sink);
  }

  /** Whether page `page_idx` has a terrain tile chunks can be mounted from. */
  page(page_idx: bigint, present: boolean): void {
    const b = BigInt.asUintN(64, page_idx);
    this.submit(0, present ? Cmd.PAGE_CREATE : Cmd.PAGE_DELETE, packet((p) => p.u64(b)));
  }

  /** Drops the chunk and its work; nothing more reaches its sink. */
  remove(id: u32): void {
    this.submit(id, Cmd.CHUNK_DELETE);
    this.sinks.delete(id);
  }

  /** What the chunk is made of: its page, or null while it has none the
   * module can read, and whether structures show. Its runs and its work
   * are dropped, and its generation moves by one. */
  source(id: u32, terrain: Terrain | null, show_structures: boolean): void {
    const page = terrain !== null && reaches(this.pxl, terrain) ? terrain : null;
    // A page past the page section is one the module cannot read, and the
    // chunk stays unbuilt: the section is sized to hold every resident page
    // (terrain/config.ts), so this is a real fault, said once.
    if (terrain !== null && page === null && !this.warned_unreachable) {
      this.warned_unreachable = true;
      console.warn(`[render/chunk] chunk ${id}'s page lies outside the core memory; its chunks cannot be built`);
    }
    const g = page?.ground;
    this.submit(
      id,
      Cmd.CHUNK_UPDATE,
      packet((p) =>
        p.branch(
          page && g ? (l) => l.u32(g.land.byteOffset).u32(g.water.byteOffset).u32(g.structure.byteOffset).u32(g.transport.byteOffset).u32(page.ele.byteOffset) : null,
        ).bool(show_structures)
      ),
    );
  }

  /** The builder's ask, past the scheduler — for the tests that hold a
   * build to the TypeScript pipeline, with `build_tick`. */
  want(id: u32, first: LodStep | null, second: LodStep | null): void {
    this.submit(id, Cmd.CHUNK_WANT, packet((p) => p.u32(first ?? 0).u32(second ?? 0)));
  }

  /** The drawn build's edits: the hidden entry by its column, band and
   * packed position, and the carve preview by its column and its corner
   * relative to it. */
  edits(hide: { column: bigint; band: number; pos: u32 } | null, carve: { column: bigint; lx: number; ly: number; lz: number; size: number } | null): void {
    this.submit(
      0,
      Cmd.EDIT_UPDATE,
      packet((p) =>
        p.branch(hide ? (h) => h.u64(BigInt.asUintN(64, hide.column)).i32(hide.band | 0).u32(hide.pos) : null)
          .branch(carve ? (c) => c.u64(BigInt.asUintN(64, carve.column)).f32(carve.lx).f32(carve.ly).f32(carve.lz).f32(carve.size) : null)
      ),
    );
  }

  /** The frame's call, once `frame` is written: every command sent so far
   * applied, the scheduler's pass, and what came of it handed to the sinks.
   * Whether the canvas has something new to draw. */
  tick(): boolean {
    const draw = (this.pxl.pxl_tick() & 1) !== 0;
    this.read();
    // What the completion ring had no room for, until it is dry: a tick
    // that finds events still waiting does nothing.
    while (this.pxl.pxl_drain() > 0) this.read();
    return draw;
  }

  /** Rebuilds chunk `id`'s layout to the end now, and hands over what that
   * made: a placement shows in the frame it was made. */
  build_now(id: u32): void {
    this.pxl.pxl_build_now(id);
    this.read();
  }

  /** Rebuilds chunk `id`'s decor for the layout it shows, now, and tells
   * the layout again; the tiers are untouched. `show_decor` is the setting
   * as it stands, since the frame is written only before a tick. */
  decor_now(id: u32, show_decor: boolean): void {
    this.pxl.pxl_decor_now(id, show_decor ? 1 : 0);
    this.read();
  }

  /** The builder alone over `ids`, for the tests: no scheduler. */
  build_tick(ids: ArrayLike<u32>, budget_ms: number): void {
    if (ids.length > this.order.length) {
      FIXED_ALLOC.give(this.order);
      this.order = FIXED_ALLOC.take(Uint32Array, 1 << Math.ceil(Math.log2(ids.length)));
    }
    for (let i = 0; i < ids.length; i++) this.order[i] = ids[i];
    this.pxl.pxl_build_tick(this.order.byteOffset, ids.length, budget_ms);
    this.read();
  }

  /** The census and the build stat are rows at fixed addresses, read
   * several times a frame: one view and one object each, filled on every
   * read — a caller keeps the numbers, never the object. */
  private readonly census_row: Float64Array;
  private readonly census_out: Census = {
    chunks: 0,
    in_frustum: 0,
    settling: 0,
    blend_active: 0,
    pending: 0,
    offscreen_pending: 0,
    oldest_visible_ms: 0,
    oldest_release_ms: 0,
    committed: 0,
    released: 0,
    retained_on: 0,
    retained_off: 0,
    pressure: 0,
  };
  private readonly stat_row: Uint32Array;
  private readonly stat_out: BuildStat = { direct: 0, scans: 0, bakes: 0, starved: 0, total_runs: 0, culled: 0, faces_possible: 0, faces_kept: 0, coarse_total: 0, coarse_low_fill: 0, heap_used: 0 };

  census(): Census {
    const c = this.census_row;
    const o = this.census_out;
    o.chunks = c[0];
    o.in_frustum = c[1];
    o.settling = c[2];
    o.blend_active = c[3];
    o.pending = c[4];
    o.offscreen_pending = c[5];
    o.oldest_visible_ms = c[6];
    o.oldest_release_ms = c[7];
    o.committed = c[8];
    o.released = c[9];
    o.retained_on = c[10];
    o.retained_off = c[11];
    o.pressure = c[12];
    return o;
  }

  stat(): BuildStat {
    const s = this.stat_row;
    const o = this.stat_out;
    o.direct = s[0];
    o.scans = s[1];
    o.bakes = s[2];
    o.starved = s[3];
    o.total_runs = s[4];
    o.culled = s[5];
    o.faces_possible = s[6];
    o.faces_kept = s[7];
    o.coarse_total = s[8];
    o.coarse_low_fill = s[9];
    o.heap_used = s[10];
    return o;
  }

  /** How often a band came from the module's stores rather than being
   * built from the pools, and how often a band was refused. A low reuse
   * share means the stores are under a composite's real band count. */
  band_stat(): { allocated: u32; reused: u32; refused: u32 } {
    const s = new Uint32Array(this.pxl.memory.buffer, this.pxl.pxl_volume_stat(), 3);
    return { allocated: s[0], reused: s[1], refused: s[2] };
  }
}

/** Whether the module can read every lane of `terrain` where it is. */
function reaches(pxl: PxlExports, terrain: Terrain): boolean {
  const memory = pxl.memory.buffer;
  return terrain.ele.buffer === memory && GROUND_CH_KEYS.every((k) => terrain.ground[k].buffer === memory);
}

function core(): CoreBuild {
  if (CORE_PXL === null) throw new Error("render: the chunks build in the module, which needs the core memory");
  return new CoreBuild(CORE_PXL);
}

/** The one channel to the module's builds. */
export const CORE_BUILD: CoreBuild = core();
