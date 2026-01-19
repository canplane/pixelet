// @xpute/spec/control/post.ts

/**
 * CommandPort — Command-based dispatcher (POST plane)
 *
 * Scope:
 * - CommandPort is a pure codec+dispatch layer for FrameKind.POST:
 *   it decodes arguments from TLV, invokes a per-command handler,
 *   and attaches the handler's result to the `body` of the response frame.
 * - CommandPort does not own transport semantics (routing, reliability,
 *   request/response direction, tx_id allocation).
 *
 * Args/Body contract:
 * - `frame.args` is a pure TLV stream representing the handler arguments.
 * - Only VectorPort/CommandPort (encode_args/decode_args) produces/consumes this format.
 *
 * Handler contract:
 * - Dispatch key is `frame.cmd` (Command).
 * - Handler signature: (args, body) -> Promise<Body | void>
 * - `void` means reply with body=null.
 *
 * Frame ownership / invariants:
 * - CommandPort MUST NOT set/clear FrameFlag.RES (router/runtime owns directionality).
 * - CommandPort does not allocate or interpret `tx_id`, `flags`, `opts`.
 * - CommandPort preserves all non-payload fields by echoing the input frame and only
 *   replacing: `errno`, `body`.
 *
 * Error mapping:
 * - Unknown Command: errno=ENOENT, error reply with body=null.
 * - In-handler errors are caught locally, logged, and converted to `body=null` as a last-resort boundary guard.
 *
 * Reliability (ACKREQ):
 * - ACKREQ semantics are owned by worker/runtime (not CommandPort).
 * - CommandPort always returns a reply frame; worker/runtime decides whether it is emitted.
 *
 * wait (Pending):
 * - Shell is not a multicast/fanout plane: each tx is expected to yield at most one reply.
 * - wait() returns body from the next POST frame of this tx.
 * - wait() is invalid on UNACK (EINVAL).
 * - wait() expects FrameKind.POST; non-POST => EBADMSG.
 * - wait() timeout/null -> ETIMEDOUT.
 */

import type { i32, u32 } from "@xpute/spec/lib/kit/word.ts";
import type { Bytes } from "@xpute/spec/lib/kit/array.ts";
import type { Async } from "@xpute/spec/lib/kit/type.ts";
import { Errno } from "@xpute/spec/sys/errno.ts";
import { XputeError } from "@xpute/spec/sys/error.ts";
import { TLVReader, type TLVValue, TLVWriter } from "@xpute/spec/io/marshal/tlv.ts";

import { type Command, FrameFlag, FrameKind, type PostFrame, type TxOptions } from "@xpute/spec/ipc/frame.ts";
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

  private async _wait(): Promise<PostFrame | null> {
    if (!(this._flags & FrameFlag.ACKREQ)) throw new XputeError(Errno.EINVAL, `post: wait() on unack`);

    const frame = await this._tx.wait(this._tx_key);
    if (!frame) return null;
    if (frame.kind !== FrameKind.POST) {
      throw new XputeError(Errno.EBADMSG, `post: wait() got non-post frame`);
    }
    return frame;
  }

  async wait(): Promise<Body> {
    const frame: PostFrame | null = await this._wait();
    if (!frame) throw new XputeError(Errno.ETIMEDOUT, `post: wait() timeout`);
    return frame.body;
  }
}

// ============ Request ============

class Request {
  constructor(
    private readonly _tx: DeviceUplink,
    private readonly _flags: u32 = FrameFlag.ACKREQ,
    private readonly _opts: TxOptions = {},
  ) {}

  // ---- Flags ----

  /** Fire-and-forget: clears ACKREQ (no RES will be emitted by worker). */
  unack(): Request {
    return new Request(this._tx, this._flags & ~FrameFlag.ACKREQ, this._opts);
  }

  // ---- Options ----

  timeout(ms: u32): Request {
    return new Request(this._tx, this._flags, { ...this._opts, timeout: ms });
  }

  // ---- Terminators (Actions) ----

  /** Lowest-level call (public; prefer read/write wrappers). */
  send<T extends Args = Args>(cmd: Command, args: T, body: Body = null): Pending {
    const tlv: Bytes = encode_args(args);

    const req: PostFrame = {
      kind: FrameKind.POST,
      tx_id: 0, // placeholder
      errno: 0,

      cmd,
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

type RxHandler<T extends Args = Args> = (args: T, body: Body) => Async<Body | void>;

export class CommandPort {
  private _cmd_table = new Map<Command, RxHandler<any>>();

  /** Request builder */
  readonly req: Request;

  constructor(tx: DeviceUplink) {
    this.req = new Request(tx);
  }

  /** Register a receive handler */
  on<T extends Args>(cmd: Command, handler: RxHandler<T>): Disposer {
    if (this._cmd_table.has(cmd)) console.warn(`post: command overwrite: '${cmd}'`);
    this._cmd_table.set(cmd, handler);

    // disposer: remove only if still mapped to the same handler
    return () => {
      if (this._cmd_table.get(cmd) === handler) this._cmd_table.delete(cmd);
    };
  }

  /**
   * Inbound request dispatch.
   *
   * - Worker layer (not CommandPort) owns ACKREQ semantics.
   * - CommandPort always dispatches and returns handler result (or body=null).
   * - Worker decides whether to emit RES frame based on ACKREQ flag.
   */
  async receive(frame: PostFrame): Promise<PostFrame> {
    const args: Args = decode_args(frame.args);

    const handler: RxHandler | undefined = this._cmd_table.get(frame.cmd);
    if (!handler) {
      console.warn(`post: unknown command: ${frame.cmd}`);
      return { ...frame, errno: Errno.ENOENT, body: null };
    }

    let res: Body;
    try {
      res = (await handler(args, frame.body)) ?? null;
    } catch (err) {
      console.error(`post: ${frame.cmd}:`, err);
      res = null;
    }
    return { ...frame, errno: 0, body: res };
  }

  /** List registered commands */
  cmds(): Command[] {
    return Array.from(this._cmd_table.keys());
  }
}
