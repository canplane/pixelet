// @xpute/gpu-web-host/belt.ts

/**
 * This is the staging belt that a guest's buffer writes go through on a page,
 * as gpu/native/host/src/belt.rs is a device's: chunks mapped for writing.
 * The host writes into a mapped chunk, and the uploads' encoder copies from
 * it into the target buffer, submitted ahead of what follows. Once submitted,
 * a chunk is mapped again, and it is free when that mapping resolves, which
 * the queue does only after the work that copies from the chunk is done.
 *
 * A device waits on its queue where every chunk is taken. A page cannot wait
 * inside a record, so it makes one chunk more, mapped as it is made, and keeps
 * it: the pool holds as many chunks as the entries' writes and the GPU's lag
 * have needed at once. The lag is bounded by the frame's admission, which
 * draws no frame while the frame latency's frames are unfinished
 * (xpute-gpu-guest `admit::frame`), so the entries that write wait as a
 * device's belt would.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { timed } from "./timed.ts";

/** The chunk is mapped, and holds nothing. */
const FREE = 0;
/** The chunk holds writes not yet submitted. */
const OPEN = 1;
/** The chunk is submitted, and is being mapped again. A chunk whose mapping
 * is refused, because the device is gone, stays so: a page never waits for a
 * chunk, and the host goes with the device. */
const BACK = 2;

/** Each mapped range begins at the mapping's alignment. */
const MAP_ALIGNMENT = 8;

interface Chunk {
  buffer: GPUBuffer;
  state: number;
  /** The chunk's mapped bytes while it is `FREE` or `OPEN`. */
  bytes: Uint8Array | null;
}

/** The room a write takes: the chunk, where in it, its mapped bytes there,
 * and the byte count. */
export interface Room {
  chunk: GPUBuffer;
  at: u32;
  bytes: Uint8Array;
  n: u32;
}

export class Belt {
  private readonly chunks: Chunk[] = [];
  /** This is the chunk being written and its first byte not yet taken. */
  private cur: { k: number; at: u32 } | null = null;

  private readonly device: GPUDevice;
  private readonly chunk_sz: u32;
  /** A remap's landing slower than this is logged (timed.ts). */
  private readonly slow_ms: u32 | null;

  constructor(device: GPUDevice, chunk_sz: u32, slow_ms: u32 | null) {
    if (chunk_sz <= 0 || chunk_sz % MAP_ALIGNMENT !== 0) {
      throw new Error(`[gpu] an upload chunk of ${chunk_sz} bytes`);
    }
    this.device = device;
    this.chunk_sz = chunk_sz;
    this.slow_ms = slow_ms;
  }

  /** This answers the room for the next `want` bytes of a write, or for as
   * many of them as the current chunk has left. */
  take(want: u32): Room {
    let k: number;
    let at: u32;
    if (this.cur && this.cur.at < this.chunk_sz) {
      ({ k, at } = this.cur);
    } else {
      k = this.chunks.findIndex((c) => c.state === FREE);
      if (k < 0) k = this.chunks.push(this.made()) - 1;
      this.chunks[k].state = OPEN;
      at = 0;
    }
    const n = Math.min(want, this.chunk_sz - at);
    const next = Math.ceil((at + n) / MAP_ALIGNMENT) * MAP_ALIGNMENT;
    this.cur = { k, at: Math.min(next, this.chunk_sz) };
    const c = this.chunks[k];
    return { chunk: c.buffer, at, bytes: c.bytes!, n };
  }

  /** Unmaps each chunk written. Call it before the uploads' encoder is
   * submitted. */
  finish(): void {
    for (const c of this.chunks) {
      if (c.state !== OPEN) continue;
      c.buffer.unmap();
      c.bytes = null;
    }
  }

  /** Maps again each chunk the uploads' encoder copies from, to be free once
   * the queue's work is done. Call it after that encoder is submitted. */
  recall(): void {
    this.cur = null;
    for (const c of this.chunks) {
      if (c.state !== OPEN) continue;
      c.state = BACK;
      c.buffer.mapAsync(GPUMapMode.WRITE).then(
        () => {
          c.bytes = timed(
            this.slow_ms,
            "remap",
            () => new Uint8Array(c.buffer.getMappedRange()),
          );
          c.state = FREE;
        },
        () => {},
      );
    }
  }

  /** One chunk more, mapped as it is made. */
  private made(): Chunk {
    const buffer = this.device.createBuffer({
      label: "upload chunk",
      size: this.chunk_sz,
      usage: GPUBufferUsage.MAP_WRITE | GPUBufferUsage.COPY_SRC,
      mappedAtCreation: true,
    });
    return {
      buffer,
      state: FREE,
      bytes: new Uint8Array(buffer.getMappedRange()),
    };
  }
}
