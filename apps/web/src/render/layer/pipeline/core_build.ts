// @pxl/web/render/layer/pipeline/core_build.ts

/**
 * The glue's half of the chunks' builds, which run in the module
 * (crates/pxl-core/src/build.rs): commands into the submission ring, one
 * `pxl_tick` a frame, and the completion ring read when it returns.
 *
 * A chunk here is an id and a sink. The node says what the chunk is made of
 * and which tiers it lacks; the module makes its runs and bakes its tiers
 * under the frame's budget and hands each mesh over whole, in a block of the
 * mesh section the sink now owns. A command is applied at the next tick, or
 * at once when the ring is full — `pump` makes room — so a sequence of them
 * is applied in the order it was sent.
 *
 * The TypeScript scan and mesher stay as the references the module is held
 * to (wasm_bake.test.ts, wasm_scan.test.ts); nothing on screen is built by
 * them.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import { Errno } from "@xpute/core/sys/errno.ts";
import { Ring, RING_ARGS, RING_OP, RING_USER } from "@xpute/core/ipc/ring.ts";
import { GROUND_CH_KEYS, type Terrain } from "@pxl/shared/bake/format.ts";

import type { LodStep } from "@/render/lod.ts";
import type { PxlExports } from "@/wasm/pxl.ts";
import { BakeContext, type BakedChunkMesh, carve_baked_mesh } from "./chunk_mesher.ts";
import { CORE_POOL, CORE_PXL, FIXED_ALLOC } from "./core_pool.ts";

/** The TypeScript bake's context over the core pool, for the references. */
export const CORE_BAKE: BakeContext = new BakeContext(CORE_POOL, FIXED_ALLOC);

/** Hands one baked mesh's block back to the core pool. Every view into it is
 * dead afterwards. */
export function release_chunk_mesh(mesh: BakedChunkMesh): void {
  CORE_POOL.release(mesh.block);
}

/** crates/pxl-wasm/src/lib.rs's op numbers. */
const OP_CHUNK_ADD = 1;
const OP_CHUNK_REMOVE = 2;
const OP_CHUNK_SOURCE = 3;
const OP_CHUNK_WANT = 4;
const OP_CHUNK_RECLAIM = 5;
const OP_EDITS = 6;
const OP_TIER = 16;
const OP_RUNS = 17;

/** What a chunk is told. */
export type ChunkBuildSink = {
  /** Tier `step`, built from the sources of `generation`; the mesh is the
   * sink's to keep or release. */
  on_tier(generation: u32, step: LodStep, mesh: BakedChunkMesh): void;
  /** The bytes the chunk's runs hold now. */
  on_runs(bytes: u32): void;
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

const $f32 = new Float32Array(1);
const $f32_bits = new Uint32Array($f32.buffer);

function f32_bits(v: number): u32 {
  $f32[0] = v;
  return $f32_bits[0];
}

export class CoreBuild {
  private readonly sq: Ring;
  private readonly cq: Ring;
  private readonly sinks = new Map<u32, ChunkBuildSink>();
  private next_id: u32 = 1;
  private order: Uint32Array;

  constructor(private readonly pxl: PxlExports) {
    this.sq = new Ring(pxl.memory.buffer, pxl.pxl_ring_sq());
    this.cq = new Ring(pxl.memory.buffer, pxl.pxl_ring_cq());
    this.order = FIXED_ALLOC.take(Uint32Array, 1024);
  }

  /** Makes room in the submission ring: applies what waits, and hands out
   * what that made. */
  pump(): void {
    this.pxl.pxl_drain();
    this.read();
  }

  private submit(user: u32, op: u32, ...args: u32[]): void {
    while (this.sq.push(user, op, ...args) === Errno.EAGAIN) this.pump();
  }

  private read(): void {
    const w = this.cq.words;
    for (let at = this.cq.peek(); at >= 0; at = this.cq.peek()) {
      const id = w[at + RING_USER];
      const op = w[at + RING_OP] & 0xffff;
      const a = at + RING_ARGS;
      const sink = this.sinks.get(id);
      if (op === OP_TIER) {
        const mesh = carve_baked_mesh(CORE_POOL.adopt(w[a + 2], w[a + 3]), w[a + 4], w[a + 5]);
        if (sink) sink.on_tier(w[a], w[a + 1] as LodStep, mesh);
        else release_chunk_mesh(mesh);
      } else if (op === OP_RUNS) {
        sink?.on_runs(w[a]);
      }
      this.cq.advance();
    }
  }

  /** A new chunk whose column's base is `base`, starting at page cell
   * (tx0, ty0): its id. */
  add(sink: ChunkBuildSink, base: bigint, tx0: u32, ty0: u32): u32 {
    const id = this.next_id++;
    this.sinks.set(id, sink);
    const b = BigInt.asUintN(64, base);
    this.submit(id, OP_CHUNK_ADD, id, Number(b & 0xffffffffn), Number(b >> 32n), tx0, ty0);
    return id;
  }

  /** Drops the chunk and its work; nothing more reaches its sink. */
  remove(id: u32): void {
    this.submit(id, OP_CHUNK_REMOVE, id);
    this.sinks.delete(id);
  }

  /** What the chunk is made of: its page, or null while it has none the
   * module can read, and whether structures show. Its runs and its work
   * are dropped, and its generation moves by one. */
  source(id: u32, terrain: Terrain | null, show_structures: boolean): void {
    const page = terrain !== null && reaches(this.pxl, terrain) ? terrain : null;
    const g = page?.ground;
    this.submit(
      id,
      OP_CHUNK_SOURCE,
      id,
      page ? 1 : 0,
      show_structures ? 1 : 0,
      g ? g.land.byteOffset : 0,
      g ? g.water.byteOffset : 0,
      g ? g.structure.byteOffset : 0,
      g ? g.transport.byteOffset : 0,
      page ? page.ele.byteOffset : 0,
    );
  }

  /** The tiers the chunk lacks, most wanted first; none gives its work back. */
  want(id: u32, first: LodStep | null, second: LodStep | null): void {
    this.submit(id, OP_CHUNK_WANT, id, first ?? 0, second ?? 0);
  }

  /** Gives back the chunk's runs and its work, keeping its sources. */
  reclaim(id: u32): void {
    this.submit(id, OP_CHUNK_RECLAIM, id);
  }

  /** The drawn build's edits: the hidden entry by its column, band and
   * packed position, and the carve preview by its column and its corner
   * relative to it. */
  edits(hide: { column: bigint; band: number; pos: u32 } | null, carve: { column: bigint; lx: number; ly: number; lz: number; size: number } | null): void {
    const split = (k: bigint): [u32, u32] => {
      const b = BigInt.asUintN(64, k);
      return [Number(b & 0xffffffffn), Number(b >> 32n)];
    };
    const [hl, hh] = hide ? split(hide.column) : [0, 0];
    const [cl, ch] = carve ? split(carve.column) : [0, 0];
    this.submit(
      0,
      OP_EDITS,
      hide ? 1 : 0,
      hl,
      hh,
      hide ? hide.band >>> 0 : 0,
      hide ? hide.pos : 0,
      carve ? 1 : 0,
      cl,
      ch,
      carve ? f32_bits(carve.lx) : 0,
      carve ? f32_bits(carve.ly) : 0,
      carve ? f32_bits(carve.lz) : 0,
      carve ? f32_bits(carve.size) : 0,
    );
  }

  /** The frame's call: every command sent so far applied, then the chunks in
   * `ids` advanced in that order for `budget_ms`, and what came of it handed
   * to their sinks. */
  tick(ids: ArrayLike<u32>, budget_ms: number): void {
    if (ids.length > this.order.length) {
      FIXED_ALLOC.give(this.order);
      this.order = FIXED_ALLOC.take(Uint32Array, 1 << Math.ceil(Math.log2(ids.length)));
    }
    for (let i = 0; i < ids.length; i++) this.order[i] = ids[i];
    this.pxl.pxl_tick(this.order.byteOffset, ids.length, budget_ms);
    this.read();
  }

  stat(): BuildStat {
    const s = new Uint32Array(this.pxl.memory.buffer, this.pxl.pxl_build_stat(), 11);
    return {
      direct: s[0],
      scans: s[1],
      bakes: s[2],
      starved: s[3],
      total_runs: s[4],
      culled: s[5],
      faces_possible: s[6],
      faces_kept: s[7],
      coarse_total: s[8],
      coarse_low_fill: s[9],
      heap_used: s[10],
    };
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
