// @pxl/gpu/host/src/belt.ts

/**
 * This is the staging belt that a guest's buffer and texture writes go
 * through on the web, as rust/gpu/host/src/belt.rs is the native one: a
 * pool of chunks mapped for writing, one more than the frame latency. The host
 * writes into a mapped chunk, and the uploads' encoder copies from it into
 * the target, submitted ahead of what follows. Once submitted, a chunk is
 * mapped again, and it is free when that mapping resolves, which the queue
 * does only after the work that copies from the chunk is done.
 *
 * The writes are packed as pxl-gpu-link `Pack` says, whose arithmetic `take`
 * holds alike: the writes staged since the last submit begin at a fresh
 * chunk, a buffer's bytes go on in the next chunk where one has no more
 * room, and a texture's rows go whole. The device's block tells the guest how
 * many of the pool's chunks are free, and the guest admits the writes that
 * may wait only as far as they hold (pxl-gpu-guest stream.rs `stage`). A
 * write that cannot wait is recorded however few are free, and where a run
 * finds none, the host makes one chunk more for it, mapped as it is made, and
 * lets it go once it is back, so the pool returns to its size; neither host
 * waits inside a record. A chunk the browser cannot make holds the device
 * lost, as a refused mapping does.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { ROOM_ALIGNMENT } from "../../link/src/api.spec.ts";

/** The chunk is mapped, and holds nothing. */
const FREE = 0;
/** The chunk holds writes not yet submitted. */
const OPEN = 1;
/** The chunk is submitted, and is being mapped again. */
const BACK = 2;
/** Its mapping was refused, or its mapped range could not be read: the
 * device is gone. */
const GONE = 3;

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
  /** The pool's chunks first, then any made for a run that found none
   * free. */
  private readonly chunks: Chunk[] = [];
  /** This is the chunk being written and its first byte not yet taken, or
   * null where the next room begins a fresh chunk. */
  private cur: { k: number; at: u32 } | null = null;

  private readonly device: GPUDevice;
  private readonly chunk_sz: u32;
  /** How many chunks the pool holds. */
  private readonly pool_cnt: number;
  private readonly back: (ok: boolean) => void;

  /** `chunk_cnt` chunks are made at once, each mapped as it is made; `back`
   * is called as each submitted chunk comes back: free, or with its mapping
   * refused, which is a lost device. */
  constructor(
    device: GPUDevice,
    chunk_sz: u32,
    chunk_cnt: u32,
    back: (ok: boolean) => void,
  ) {
    if (chunk_sz <= 0 || chunk_sz % ROOM_ALIGNMENT !== 0) {
      throw new Error(`[gpu] an upload chunk of ${chunk_sz} bytes`);
    }
    this.device = device;
    this.chunk_sz = chunk_sz;
    this.pool_cnt = chunk_cnt;
    this.back = back;
    for (let k = 0; k < chunk_cnt; k++) this.chunks.push(this.make_chunk());
  }

  /** How many of the pool's chunks are free, which the device's block tells
   * the guest (spec DEVICE). */
  free_cnt(): u32 {
    return this.chunks.slice(0, this.pool_cnt).filter((c) => c.state === FREE)
      .length;
  }

  /** This answers the room for the next `want` bytes of a write, or for as
   * many whole rows of `row_sz` bytes of them as the current chunk has left. A
   * buffer's write is rows of one byte. A room that begins a fresh chunk takes
   * a free chunk of the pool, or one made for it where none is free. It
   * answers null where the device is gone: a chunk's mapping was refused, a
   * chunk could not be made, or the room's mapped bytes are no longer the
   * chunk's, which any loss may take away. */
  take(want: u32, row_sz: u32): Room | null {
    if (row_sz <= 0 || row_sz > this.chunk_sz) {
      throw new Error(
        `[gpu] a row of ${row_sz} bytes in an upload chunk of ${this.chunk_sz}`,
      );
    }
    if (this.chunks.some((c) => c.state === GONE)) return null;
    let k: number;
    let at: u32;
    if (this.cur && this.chunk_sz - this.cur.at >= row_sz) {
      ({ k, at } = this.cur);
    } else {
      this.let_go();
      k = this.chunks
        .slice(0, this.pool_cnt)
        .findIndex((c) => c.state === FREE);
      if (k < 0) {
        let chunk: Chunk;
        try {
          chunk = this.make_chunk();
        } catch {
          return null;
        }
        k = this.chunks.push(chunk) - 1;
      }
      this.chunks[k].state = OPEN;
      at = 0;
    }
    const c = this.chunks[k];
    if (c.bytes?.length !== this.chunk_sz) return null;
    const left = this.chunk_sz - at;
    const n = Math.min(want, left - (left % row_sz));
    const next = Math.ceil((at + n) / ROOM_ALIGNMENT) * ROOM_ALIGNMENT;
    this.cur = { k, at: Math.min(next, this.chunk_sz) };
    return { chunk: c.buffer, at, bytes: c.bytes, n };
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
   * the queue's work is done, and begins the next room at a fresh chunk.
   * Call it after that encoder is submitted. */
  recall(): void {
    this.cur = null;
    for (const c of this.chunks) {
      if (c.state !== OPEN) continue;
      c.state = BACK;
      c.buffer
        .mapAsync(GPUMapMode.WRITE)
        .then(() => {
          c.bytes = new Uint8Array(c.buffer.getMappedRange());
          c.state = FREE;
        })
        .then(
          () => this.back(true),
          () => {
            c.state = GONE;
            this.back(false);
          },
        );
    }
    this.let_go();
  }

  /** One chunk more, mapped as it is made. It throws where the browser
   * cannot make it. */
  private make_chunk(): Chunk {
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

  /** Lets go each chunk past the pool that is back, so the pool returns to
   * its size. Call it only where no room is being written. */
  private let_go(): void {
    for (let k = this.pool_cnt; k < this.chunks.length;) {
      if (this.chunks[k].state !== FREE) {
        k++;
        continue;
      }
      this.chunks[k].buffer.destroy();
      this.chunks[k] = this.chunks[this.chunks.length - 1];
      this.chunks.pop();
    }
  }
}
