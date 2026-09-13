// @pxl/web/module/world/sched/bake_port.ts

/**
 * The main thread's side of the bake worker: a request per borrowed slab,
 * the replies held until their chunk adopts them, and nothing else.
 *
 * Not a Demand through the fetch scheduler. Its quota is the network's six
 * connections and its `run` is a Promise; a bake is CPU on another core,
 * capped by the slabs, and its completion is read by the tick rather than
 * delivered to a continuation. So the table is the slabs themselves: a slab
 * away is a request away, its tx_id is the only correlation, and a reply
 * waits in its slot until the chunk that asked polls and adopts it under
 * the frame's budget. There is no deadline and no retry — a worker in a
 * page does not lose messages — and a worker that dies fails every request
 * it held, which its chunks see on their next poll and simply ask again
 * (issue/20260913-boundaries-commands-and-what-comes-back-from-april.md).
 *
 * Cancellation leaves a request away; its reply is dropped on arrival and
 * the slab goes home then. A chunk never waits on a cancelled bake.
 */

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import type { BuddyAllocator } from "@xpute/core/ds/buddy.ts";
import { Frame, type FrameMessage, make_tx_id } from "@xpute/core/ipc/frame.ts";
import { endpoint_link, type Link } from "@xpute/core/ipc/link.ts";

import type { LodStep } from "@/render/lod.ts";
import type { ColumnRunSet } from "@/render/layer/pipeline/column_run.ts";
import { type BakeReply, read_bake_reply, reply_mesh_bytes, write_bake_request } from "@/render/layer/pipeline/bake_command.ts";
import { little_slabs, type LittleSlabs } from "./slab.ts";

export type BakeWorker = { link: Link; terminate(): void; on_error(fail: () => void): void };
export type SpawnBakeWorker = () => BakeWorker;

/**
 * Mesh bytes adopted per tick before the rest wait for the next one, while
 * WebGL is the graphics path. A bake on the main thread was rate-limited by
 * the frame budget; off it, a zoom-out can finish faster than Safari's WebGL
 * path copes with uploads, which is where its memory balloons
 * (issue/20260912-why-the-destination-is-wasm.md, 2026-09-13). A knob, not a
 * measurement: the phone reading sets it, and one mesh is always adopted
 * whatever its size, so nothing starves.
 */
export const ADOPT_BYTES_PER_TICK: u32 = 8 << 20;

const BAKE_WORKER_ID: u32 = 1;

const FREE = 0;
const AWAY = 1;
const READY = 2;
const CANCELLED = 3;
const FAILED = 4;

export type BakePoll = "away" | "ready" | "failed";

export type BakePortStats = {
  submitted: u32;
  adopted: u32;
  spilled: u32;
  cancelled: u32;
  failed: u32;
  deferred: u32;
  away_ms: f64;
  away_max_ms: f64;
  adopt_ms: f64;
  adopt_max_ms: f64;
};

export class BakePort {
  private worker: BakeWorker | null = null;
  private seq: u32 = 0;
  /** Per slab id: the slot's state, its tx_id, when it left, and its reply. */
  private readonly state: Uint8Array;
  private readonly tx: Uint32Array;
  private readonly sent_ms: Float64Array;
  private readonly replies: (FrameMessage | null)[];
  private readonly jobs: string[];
  private adopted_bytes: u32 = 0;
  readonly stats: BakePortStats = { submitted: 0, adopted: 0, spilled: 0, cancelled: 0, failed: 0, deferred: 0, away_ms: 0, away_max_ms: 0, adopt_ms: 0, adopt_max_ms: 0 };

  constructor(private readonly spawn: SpawnBakeWorker, private readonly slabs: LittleSlabs = little_slabs) {
    this.state = new Uint8Array(slabs.cap);
    this.tx = new Uint32Array(slabs.cap);
    this.sent_ms = new Float64Array(slabs.cap);
    this.replies = new Array(slabs.cap).fill(null);
    this.jobs = new Array(slabs.cap).fill("");
  }

  /** Called once at the start of each tick's commit pass. */
  begin_tick(): void {
    this.adopted_bytes = 0;
  }

  /**
   * Sends a bake of `set` at `step` for `job`, or returns null when every
   * slab is away — the request waits, the way a fetch waits for a slot. The
   * run set is copied into the slab here, so the caller may release it.
   */
  submit(job: string, set: ColumnRunSet, step: LodStep): u32 | null {
    const slab = this.slabs.borrow(job);
    if (slab === null) return null;
    const worker = this.ensure_worker();
    const tx_id = make_tx_id(BAKE_WORKER_ID, ++this.seq);
    const spill = write_bake_request(new Frame(slab.buffer), tx_id, set, step);
    this.state[slab.id] = AWAY;
    this.tx[slab.id] = tx_id;
    this.sent_ms[slab.id] = performance.now();
    this.jobs[slab.id] = job;
    this.stats.submitted++;
    if (spill) this.stats.spilled++;
    worker.link.send({ frame: slab.buffer, spill });
    return tx_id;
  }

  poll(tx_id: u32): BakePoll {
    const id = this.find(tx_id);
    if (id < 0) return "failed";
    const s = this.state[id];
    return s === READY ? "ready" : s === AWAY ? "away" : "failed";
  }

  /** Whether a ready reply fits what is left of this tick's upload bytes.
   * The first adoption of a tick always fits. Counts a refusal. */
  can_adopt(tx_id: u32): boolean {
    const id = this.find(tx_id);
    if (id < 0 || this.state[id] !== READY) return false;
    if (this.adopted_bytes === 0) return true;
    const fits = this.adopted_bytes + reply_mesh_bytes(new Frame(this.replies[id]!.frame)) <= ADOPT_BYTES_PER_TICK;
    if (!fits) this.stats.deferred++;
    return fits;
  }

  /**
   * Copies a ready reply's mesh into `pool` and sends the slab home. Null
   * when the worker replied with an error: the slab is home all the same,
   * and the caller asks again.
   */
  adopt(tx_id: u32, pool: BuddyAllocator): BakeReply | null {
    const id = this.find(tx_id);
    if (id < 0 || this.state[id] !== READY) return null;
    const msg = this.replies[id]!;
    const t0 = performance.now();
    let reply: BakeReply | null = null;
    try {
      reply = read_bake_reply(new Frame(msg.frame), msg.spill, pool);
    } catch (err) {
      console.error("[sched/bake] worker replied with an error:", err);
      this.stats.failed++;
    }
    this.free(id, msg.frame);
    if (reply) {
      const ms = performance.now() - t0;
      this.stats.adopted++;
      this.stats.adopt_ms += ms;
      if (ms > this.stats.adopt_max_ms) this.stats.adopt_max_ms = ms;
      this.adopted_bytes += reply.mesh.block.bytes;
    }
    return reply;
  }

  /** The asker no longer wants this bake. A ready or failed slot is freed
   * now; one still away is freed when its reply arrives. */
  cancel(tx_id: u32): void {
    const id = this.find(tx_id);
    if (id < 0) return;
    const s = this.state[id];
    if (s === AWAY) {
      this.state[id] = CANCELLED;
      this.stats.cancelled++;
    } else if (s === READY) {
      this.free(id, this.replies[id]!.frame);
      this.stats.cancelled++;
    } else if (s === FAILED) {
      this.state[id] = FREE;
    }
  }

  private find(tx_id: u32): number {
    for (let i = 0; i < this.tx.length; i++) if (this.state[i] !== FREE && this.tx[i] === tx_id) return i;
    return -1;
  }

  private free(id: number, frame: ArrayBuffer): void {
    this.slabs.give_back(id, frame);
    this.state[id] = FREE;
    this.replies[id] = null;
    this.jobs[id] = "";
  }

  private ensure_worker(): BakeWorker {
    if (this.worker) return this.worker;
    const worker = this.spawn();
    worker.link.on((msg) => this.receive(msg));
    worker.on_error(() => this.lose(worker));
    this.worker = worker;
    return worker;
  }

  private receive(msg: FrameMessage): void {
    const id = this.find(new Frame(msg.frame).tx_id);
    if (id < 0) return; // its slab was forgotten when a worker was lost
    if (this.state[id] === CANCELLED) {
      this.free(id, msg.frame);
      return;
    }
    if (this.state[id] !== AWAY) return;
    const ms = performance.now() - this.sent_ms[id];
    this.stats.away_ms += ms;
    if (ms > this.stats.away_max_ms) this.stats.away_max_ms = ms;
    this.state[id] = READY;
    this.replies[id] = msg;
  }

  /** A worker died: every request it held fails and its slabs are written
   * off, and the next submit starts a fresh worker. */
  private lose(worker: BakeWorker): void {
    if (this.worker !== worker) return;
    console.error("[sched/bake] bake worker lost; its requests fail and the next bake starts another");
    for (let id = 0; id < this.state.length; id++) {
      const s = this.state[id];
      if (s !== AWAY && s !== CANCELLED) continue;
      this.slabs.forget(this.jobs[id]);
      this.state[id] = s === AWAY ? FAILED : FREE;
      if (s === AWAY) this.stats.failed++;
      this.jobs[id] = "";
    }
    worker.terminate();
    this.worker = null;
  }
}

function spawn_bake_worker(): BakeWorker {
  const worker = new Worker(new URL("../../../render/layer/pipeline/bake_worker.ts", import.meta.url), { type: "module" });
  return {
    link: endpoint_link(worker),
    terminate: () => worker.terminate(),
    on_error: (fail) => {
      worker.addEventListener("error", fail);
      worker.addEventListener("messageerror", fail);
    },
  };
}

/** The one bake port, beside the one little memory. Spawns nothing until the
 * first bake is sent. */
export const bake_port = new BakePort(spawn_bake_worker);
