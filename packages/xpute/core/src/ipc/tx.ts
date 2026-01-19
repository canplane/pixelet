// @xpute/core/ipc/tx.ts

/**
 * TxController — per-worker transaction demux + bounded mailbox
 *
 * Scope:
 * - Assigns tx_id, tracks outstanding ACKREQ transactions, and demultiplexes inbound frames by tx_id.
 * - Provides a single-consumer wait() API with bounded per-tx and global queues.
 * - Owns timeout/EoF semantics for pending transactions.
 *
 * tx_id layout:
 * - tx_id = (worker_id << WORKER_ID_SHAMT) | tx_key
 * - tx_key is a monotonic sequence masked by SEQ_MASK (wraps).
 * - Wrap collision (tx_key already live) is an invariant break and returns EAGAIN.
 *
 * Reliability (ACKREQ):
 * - ACKREQ=0 (UNACK): send() forwards frame immediately; no tx_table entry is created.
 * - ACKREQ=1 (ACKREQ): send() installs TxState (queue + optional waiter + timer) and forwards frame.
 *
 * wait() contract:
 * - wait(tx_key) resolves to the next inbound DataFrame for that tx, or null on timeout EOF.
 * - At most one outstanding waiter is allowed per tx (double wait => EBUSY).
 * - If a queued event exists, wait() returns immediately (FIFO).
 * - Negative tx_key is treated as errno from send() and rejects with XputeError(tx_key).
 * - Unknown tx_key rejects with XputeError(ENOENT).
 *
 * receive() contract:
 * - Demux key: tx_key = frame.tx_id & SEQ_MASK.
 * - If tx is unknown: frame is dropped (orphan).
 * - If frame.errno != 0: it is converted to XputeError and delivered/queued as an error event.
 * - Delivery priority: waiter first; otherwise enqueue.
 * - Enqueue is bounded:
 *   - Per-tx capacity: Q_CAP_PER_TX
 *   - Global capacity: Q_CAP_TOTAL
 *   - On overflow: drop new event (backpressure via loss).
 *
 * Timeout / cleanup:
 * - Each ACKREQ tx installs a timer (opts.timeout or this.default_timeout).
 * - On timer fire:
 *   - If a waiter is pending and the per-tx queue is empty, resolve waiter with null.
 *   - Then cleanup() runs: clears queue, cancels timer, removes tx entry (hard EOF).
 *
 * Resource limits:
 * - TX_WAIT_CAP bounds the number of live ACKREQ transactions (send() returns EAGAIN when full).
 * - Q_CAP_TOTAL / Q_CAP_PER_TX bound queued events (drops on overflow).
 */

import type { i32, u32 } from "@xpute/spec/lib/kit/word.ts";
import { Errno } from "@xpute/spec/sys/errno.ts";
import { XputeError } from "@xpute/spec/sys/error.ts";
import { Deque } from "@xpute/spec/lib/ds/deque.ts";

import type { DeviceUplink } from "@xpute/spec/ipc/link.ts";
import { type DataFrame, FrameFlag, type WorkerLink } from "@xpute/spec/ipc/frame.ts";
import { WORKER_ID_SHAMT, WorkerID } from "@xpute/spec/ipc/topology.ts";

type TxWaiter = {
  resolve: (frame: DataFrame | null) => void;
  reject: (err: unknown) => void;
};
type TxEvent = DataFrame | XputeError;

type TxState = {
  q: Deque<TxEvent>;
  waiter: TxWaiter | null;
  timer: ReturnType<typeof setTimeout>;
};

/**
 * Resource limits:
 * - TX_WAIT_CAP: Max concurrent ACKREQ transactions across all WASM instances/tasks
 *   managed by this controller. If exceeded, send() returns EAGAIN.
 * - Q_CAP_TOTAL: Global mailbox capacity for all transactions. Prevents
 *   memory exhaustion if many TXs have unconsumed response queues.
 * - Q_CAP_PER_TX: Max response frames queued for a single transaction (e.g., fanout).
 *   New frames are dropped after this limit to provide backpressure via loss.
 */
const TX_WAIT_CAP: u32 = 1 << 12; // 4096 effective IDs
const Q_CAP_TOTAL = 1 << 15; // Global Q_CAP (32,768 frames)
const Q_CAP_PER_TX = 1 << 6; // Per-TX Q_CAP (64 frames)

// tx_id layout: [ worker_id (8b) | tx_key (24b) ]
// 24-bit sequence provides 16.7M unique keys per worker.
const SEQ_MASK: u32 = (1 << WORKER_ID_SHAMT) - 1;

export type TxControllerOptions = {
  default_timeout?: u32;
};

export class TxController implements DeviceUplink {
  private readonly _worker_mask: u32;

  readonly default_timeout: u32;

  private readonly _tx_table = new Map<i32, TxState>();
  private _tx_key: i32 = 0;
  private _q_total: u32 = 0;

  constructor(
    readonly worker_id: WorkerID,
    readonly uplink: WorkerLink,
    opts: TxControllerOptions = {},
  ) {
    if (!(0 < worker_id && worker_id <= WorkerID.GUEST)) {
      throw new Error(`tx: invalid worker id: ${worker_id}: must be 1-${WorkerID.GUEST}`);
    }
    this._worker_mask = worker_id << WORKER_ID_SHAMT;

    const { default_timeout = 5000 } = opts;
    this.default_timeout = default_timeout;
  }

  // Monotonic increment within 24-bit space.
  // We don't use 0 as a valid key to avoid ambiguity with uninitialized states.
  private generate_tx_key(): i32 {
    this._tx_key = (this._tx_key + 1) & SEQ_MASK;
    if (this._tx_key === 0) this._tx_key = 1;
    return this._tx_key;
  }

  send<T extends DataFrame>(frame: T): i32 {
    const { flags, opts } = frame;
    const { timeout = this.default_timeout } = opts;

    const tx_key: i32 = this.generate_tx_key();
    /**
     * INVARIANT: In a 24-bit sequence space with TX_WAIT_CAP=4096, a collision
     * implies a catastrophic leak or a 16M+ wrap-around without clearing old TXs.
     * We return EAGAIN to signal that the transaction table is effectively unusable.
     */
    if (this._tx_table.has(tx_key)) return Errno.EAGAIN; // wrap collision / zombie guard (invariant break)

    frame.tx_id = this._worker_mask | (tx_key as u32);

    // ---- Best effort ----

    if (!(flags & FrameFlag.ACKREQ)) {
      this.uplink.send(frame);
      return tx_key;
    }

    // ---- Reliable transfer ----

    if (this._tx_table.size >= TX_WAIT_CAP) return Errno.EAGAIN; // transaction table full

    const tx: TxState = {
      q: new Deque<TxEvent>(new Array<TxEvent | undefined>(Q_CAP_PER_TX)),
      waiter: null,
      timer: setTimeout(() => {
        const tx: TxState | undefined = this._tx_table.get(tx_key);
        if (!tx) return;

        if (tx.waiter && tx.q.empty()) {
          const waiter = tx.waiter;
          tx.waiter = null;
          waiter.resolve(null);
        }

        this.cleanup(tx_key, tx); // hard EOF: drop all queued events
      }, timeout),
    };

    this._tx_table.set(tx_key, tx);
    this.uplink.send(frame);
    return tx_key;
  }

  wait(tx_key: i32): Promise<DataFrame | null> {
    // negative value is errno
    if (tx_key < 0) return Promise.reject(new XputeError(tx_key, `tx: send failed: errno=${tx_key}`));

    const tx: TxState | undefined = this._tx_table.get(tx_key);
    if (!tx) return Promise.reject(new XputeError(Errno.ENOENT, `tx: unknown transaction: 0x${tx_key.toString(16)}`));

    // at most 1 waiter per tx_id
    if (tx.waiter) return Promise.reject(new XputeError(Errno.EBUSY, `tx: double wait transaction: 0x${tx_key.toString(16)}`));

    if (!tx.q.empty()) {
      const e: TxEvent = tx.q.pop_front();
      this._q_total--;
      return e instanceof XputeError ? Promise.reject(e) : Promise.resolve(e);
    }
    return new Promise((resolve, reject) => {
      tx.waiter = { resolve, reject };
    });
  }

  receive<T extends DataFrame>(frame: T): void {
    const tx_key: i32 = (frame.tx_id & SEQ_MASK) as i32;
    const tx: TxState | undefined = this._tx_table.get(tx_key);
    if (!tx) {
      // NOTE: An orphan could be a late-arriving frame for an already cleaned-up TX.
      console.warn(`tx: orphan transaction: 0x${frame.tx_id.toString(16)}`);
      return;
    }

    const e: TxEvent = frame.errno ? new XputeError(frame.errno, `tx: remote error: errno=${frame.errno}`) : frame;

    // deliver to waiter
    if (tx.waiter) {
      const waiter: TxWaiter = tx.waiter;
      tx.waiter = null; // Mark waiter as fulfilled

      /**
       * DESIGN CHOICE: Eager vs Lazy Cleanup
       * In multicast/fanout scenarios (1:N), a single tx_key can receive multiple
       * response frames. We MUST keep the TX entry alive until the timeout expires.
       *
       * 1. It allows wait_next() to drain multiple responses from the queue (tx.q).
       * 2. The timeout acts as the definitive EOF for the response stream.
       * 3. It prevents tx_key collision during the in-flight window of the fanout.
       */
      e instanceof XputeError ? waiter.reject(e) : waiter.resolve(e);
      return;
    }

    // enqueue (drop if full)
    if (this._q_total >= Q_CAP_TOTAL || tx.q.full()) return;
    tx.q.push_back(e);
    this._q_total++;
  }

  private cleanup(tx_key: i32, tx: TxState): void {
    this._q_total -= tx.q.size();
    tx.q.clear();
    clearTimeout(tx.timer);
    this._tx_table.delete(tx_key);
  }
}
