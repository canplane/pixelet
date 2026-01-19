// @xpute/spec/control/signal.ts

/**
 * VectorPort — Vector-based dispatcher (SIGNAL plane)
 *
 * Scope:
 * - VectorPort is a pure codec+dispatch layer for FrameKind.SIGNAL:
 *   it decodes arguments from TLV, invokes a per-vector handler,
 *   and attaches the handler's result to the `body` of the response frame.
 *
 * Selector (sel) contract:
 * - `sel` is delivered verbatim from the received frame (no rewriting).
 * - `sel` meaning is transport-defined; VectorPort treats it as opaque routing metadata.
 *
 * Args/Body contract:
 * - `frame.args` is a pure TLV stream representing the handler arguments.
 * - Only VectorPort/CommandPort (encode_args/decode_args) produces/consumes this format.
 *
 * Handler contract:
 * - Dispatch key is `frame.vec` (Vector).
 * - Handler signature: (sel, args, body) -> Promise<Body | void>
 * - `void` means reply with body=null.
 *
 * Frame ownership / invariants:
 * - VectorPort MUST NOT set/clear FrameFlag.RES (router/runtime owns directionality).
 * - VectorPort does not allocate or interpret `tx_id`, `rid`, `flags`, `opts`.
 * - VectorPort preserves all non-payload fields by echoing the input frame and only
 *   replacing: `errno`, `body`.
 *
 * RID contract:
 * - `rid` is request identity, generated at send-time (typically from vec + request TLV args).
 * - Replies MUST echo the inbound `rid`; reply payload MUST NOT affect `rid`.
 * - Therefore reply `body` is result payload only and is not part of identity.
 *
 * Error mapping:
 * - Unknown Vector: errno=ENOENT, error reply with body=null.
 * - In-handler errors are caught locally, logged, and converted to `body=null` as a last-resort boundary guard.
 *
 * Reliability (ACKREQ):
 * - ACKREQ semantics are owned by worker/runtime (not VectorPort).
 * - VectorPort always returns a reply frame; worker/runtime decides whether it is emitted.
 *
 * wait_next / wait (Pending):
 * - wait_next() returns [sel, body] from the next SIGNAL frame of this tx,
 *   or null on EOF/timeout (stream-friendly for multicast/fanout).
 * - wait_next() is invalid on UNACK (EINVAL).
 * - wait() is a convenience wrapper: null -> ETIMEDOUT (single-result path).
 */

import type { i32, u32 } from "@xpute/spec/lib/kit/word.ts";
import type { Bytes } from "@xpute/spec/lib/kit/array.ts";
import type { Async } from "@xpute/spec/lib/kit/type.ts";
import { Errno } from "@xpute/spec/sys/errno.ts";
import { XputeError } from "@xpute/spec/sys/error.ts";
import { TLVReader, type TLVValue, TLVWriter } from "@xpute/spec/io/marshal/tlv.ts";

import { FrameFlag, FrameKind, type SignalFrame, type TxOptions, type Vector } from "@xpute/spec/ipc/frame.ts";
import { FANOUT, IS_ROLE_SEL, Node, type NodeSelector } from "@xpute/spec/rpc/node.ts";
import type { RID } from "@xpute/spec/rpc/rid.ts";
import type { DeviceUplink } from "@xpute/spec/ipc/link.ts";

type Disposer = () => void;

type Args = TLVValue[];
type Body = Nullable<Bytes>;

function encode_args(args: Args): Bytes {
  const w = new TLVWriter();
  w.write_all(args);
  return w.finish();
}
function decode_args(tlv: Bytes): Args {
  const r = new TLVReader(tlv);
  return Array.from(r);
}

// ============ Pending ============

class Pending {
  constructor(
    private readonly _tx: DeviceUplink,
    private readonly _tx_key: i32,
    private readonly _flags: u32,
  ) {}

  /**
   * @internal
   * Low-level waiter. Returns null only on timeout (EOF).
   * Throws on invalid frame types or protocol violations.
   */
  private async _wait(): Promise<SignalFrame | null> {
    if (!(this._flags & FrameFlag.ACKREQ)) throw new XputeError(Errno.EINVAL, `signal: wait() on unack`);

    const frame = await this._tx.wait(this._tx_key);
    if (!frame) return null;
    if (frame.kind !== FrameKind.SIGNAL) {
      throw new XputeError(Errno.EBADMSG, `signal: wait() got non-signal frame`);
    }
    return frame;
  }

  /**
   * Strict waiter: returns only the body.
   * @throws {XputeError} ETIMEDOUT if the request expires.
   */
  async wait(): Promise<Body> {
    const frame: SignalFrame | null = await this._wait();
    if (!frame) throw new XputeError(Errno.ETIMEDOUT, `signal: wait() timeout`);
    return frame.body;
  }

  /**
   * Permissive waiter: useful for optional responses or polling.
   * @returns [sender, body] or null if the transaction timed out.
   */
  async wait_next(): Promise<[NodeSelector, Body] | null> {
    const frame: SignalFrame | null = await this._wait();
    if (!frame) return null;
    return [frame.sel, frame.body];
  }
}

// ============ Request ============

class Request {
  constructor(
    private readonly _tx: DeviceUplink,
    private readonly _sel: NodeSelector = Node.LOOPBACK,
    private readonly _rid: RID | null = null,
    private readonly _flags: u32 = FrameFlag.ACKREQ,
    private readonly _opts: TxOptions = {},
  ) {}

  /** Set destination selector and optional fanout hint. */
  to(sel: NodeSelector, fanout?: u32): Request {
    if (IS_ROLE_SEL(sel)) fanout ??= FANOUT.ONE;
    return new Request(this._tx, sel, this._rid, this._flags, { ...this._opts, fanout });
  }

  rid(rid: RID): Request {
    return new Request(this._tx, this._sel, rid, this._flags, this._opts);
  }

  // ---- Flags ----

  /** Fire-and-forget: clears ACKREQ (no RES will be emitted by worker). */
  unack(): Request {
    return new Request(this._tx, this._sel, this._rid, this._flags & ~FrameFlag.ACKREQ, this._opts);
  }

  cache(): Request {
    return new Request(this._tx, this._sel, this._rid, this._flags | FrameFlag.CACHE, this._opts);
  }

  // ---- Options ----

  timeout(ms: u32): Request {
    return new Request(this._tx, this._sel, this._rid, this._flags, { ...this._opts, timeout: ms });
  }

  // ---- Terminators (Actions) ----

  /** Lowest-level call (public; prefer read/write wrappers). */
  send<T extends Args = Args>(vec: Vector, args: T, body: Body = null): Pending {
    const tlv: Bytes = encode_args(args);

    const req: SignalFrame = {
      kind: FrameKind.SIGNAL,
      tx_id: 0, // placeholder
      errno: 0,

      sel: this._sel,
      rid: this._rid,
      vec,
      flags: this._flags,
      opts: this._opts,

      args: tlv,
      body,
    };
    const tx_key: i32 = this._tx.send(req);

    return new Pending(this._tx, tx_key, this._flags);
  }
}

// ============ Controller ============

type RxHandler<T extends Args = Args> = (sel: NodeSelector, args: T, body: Body) => Async<Body | void>;

export class VectorPort {
  private _vec_table = new Map<Vector, RxHandler<any>>();

  /** Request builder */
  readonly req: Request;

  constructor(tx: DeviceUplink) {
    this.req = new Request(tx);
  }

  /** Register a receive handler */
  on<T extends Args>(vec: Vector, handler: RxHandler<T>): Disposer {
    if (this._vec_table.has(vec)) console.warn(`signal: vector overwrite: ${vec}`);
    this._vec_table.set(vec, handler);

    // disposer: remove only if still mapped to the same handler
    return () => {
      if (this._vec_table.get(vec) === handler) this._vec_table.delete(vec);
    };
  }

  /**
   * Inbound request dispatch.
   *
   * - Worker layer (not VectorPort) owns ACKREQ semantics.
   * - VectorPort always dispatches and returns handler result (or body=null).
   * - VectorPort/device MUST NOT touch FrameFlag.RES.
   */
  async receive(frame: SignalFrame): Promise<SignalFrame> {
    const args: Args = decode_args(frame.args);

    const handler: RxHandler | undefined = this._vec_table.get(frame.vec);
    if (!handler) {
      console.warn(`signal: unknown vector: ${frame.vec}`);
      return { ...frame, errno: Errno.ENOENT, body: null };
    }

    let res: Body;
    try {
      res = (await handler(frame.sel, args, frame.body)) ?? null;
    } catch (err) {
      console.error(`signal: ${frame.vec}:`, err);
      res = null;
    }
    return { ...frame, errno: 0, body: res };
  }

  /** List registered vectors */
  vecs(): Vector[] {
    return Array.from(this._vec_table.keys());
  }
}
