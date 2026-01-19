// @xpute/spec/ipc/frame.ts

/**
 * Frame Hierarchy
 *
 * Frame
 * ├─ ControlFrame (WorkerInit | LinkInitSyn | LinkInitAck)
 * └─ DataFrame (SignalFrame | PostFrame)
 */

import type { u16, u32 } from "@xpute/spec/lib/kit/word.ts";
import type { U8Array } from "@xpute/spec/lib/kit/array.ts";
import { Errno } from "@xpute/spec/sys/errno.ts";

import { DevicePort, WorkerID } from "@xpute/spec/ipc/topology.ts";
import type { NodeSelector } from "@xpute/spec/rpc/node.ts";
import type { RID } from "@xpute/spec/rpc/rid.ts";

export type Vector = u16;
export type Command = string;

/**
 * Frame kind discriminator
 * - Control plane: 0x80+ (worker/link management)
 * - Data plane: 0x01-0x7F (application payloads)
 */
const CTRL_MASK = 0x80;
export const enum FrameKind {
  // Control Plane
  WORKER_READY = CTRL_MASK | 0x00, // or next free
  WORKER_INIT = CTRL_MASK | 0x01,
  LINK_INIT_SYN = CTRL_MASK | 0x02,
  LINK_INIT_ACK = CTRL_MASK | 0x03,

  // Data Plane
  SIGNAL = 0x01,
  POST = 0x02,
}

/**
 * Single unified link for all frame kinds.
 * Control and data frames share the same transport channel.
 */
export type WorkerLink = {
  send(frame: Frame): void;
  on(handler: (frame: Frame) => void): () => void;
};

export type WorkerAttr = {};

// ============ Control Plane ============

export type WorkerReady = { kind: FrameKind.WORKER_READY };
export type WorkerInit = WorkerAttr & {
  kind: FrameKind.WORKER_INIT;

  worker_id: WorkerID;
};

export type WorkerArg = WorkerAttr & {
  worker_id: WorkerID;
  uplink: WorkerLink;
};

/**
 * Link Initialization Protocol
 * Handshake between host and worker to establish routing tables
 */

export type LinkInitSyn = {
  kind: FrameKind.LINK_INIT_SYN;

  port: DevicePort;
  capable_vecs: Vector[]; // Capability vectors this worker handles
  capable_cmds: Command[]; // Commands this worker handles
};

export type LinkInitAck = {
  kind: FrameKind.LINK_INIT_ACK;

  vec_table: [Vector, DevicePort][]; // vec -> port mapping
  cmd_table: [Command, DevicePort][]; // cmd -> port mapping
};

export type ControlFrame = WorkerReady | WorkerInit | LinkInitSyn | LinkInitAck;

// ============ Transaction Options (layer-specific hints) ============

// Local transfer hint
export type TxOptions = {
  // tx controller hint
  timeout?: u32; // Hard deadline: 이 시간이 지나면 큐에 남은 데이터도 모두 폐기됩니다.

  // network gateway hint
  fanout?: u32; // Multicast count
};

/**
 * Operation Flags (transport-layer control bits)
 */
export const enum FrameFlag {
  // RES: response bit (REQ is implicit when bit=0)
  RES = 0x01,

  // Reliability
  ACKREQ = 0x10, // Require acknowledgement (reliable delivery)

  // Network hints (gateway / transport interprets these)
  //
  // CACHE is a RID-addressable fetch hint.
  //
  // It does NOT mean "always use HTTP", and it does NOT replace the normal
  // WS/mesh signaling path. The signaling/control frame still flows normally;
  // CACHE only adds an optional fetch-style consumption semantic around RID.
  //
  // Canonical meaning:
  // - The payload/resource identified by (rid) may be treated as a stable,
  //   fetchable object by transports that support split wire/store delivery
  //   (e.g. CENTRAL ws + http outbox).
  // - Transport MAY ignore this bit if the transport has no cache/fetch plane
  //   (e.g. current MESH/WebRTC path).
  //
  // Directional policy:
  //
  // 1) client -> server
  //    a) body == null:
  //       - CACHE is meaningful.
  //       - The request is a deterministic RID-addressable lookup.
  //       - WS signaling still carries the request frame.
  //       - A transport MAY additionally use/prefetch/serve an HTTP GET-style
  //         path keyed by rid, if its policy supports that.
  //
  //    b) body != null:
  //       - CACHE is ignored.
  //       - Body-bearing requests are not treated as cacheable fetch objects.
  //
  // 2) server -> client
  //    a) body == null:
  //       - CACHE is nonsensical / no-op.
  //       - There is nothing to fetch, so no RID fetch semantic is created.
  //
  //    b) body != null:
  //       - CACHE is meaningful.
  //       - The sender MAY stage the payload into an outbox/store under rid,
  //         and use WS only as the notify/control path.
  //       - The receiver may then consume the payload by RID-based GET/pull,
  //         instead of relying on inline body delivery semantics.
  //
  // Practical rule:
  // - CACHE is only meaningful when the payload shape and direction make RID-
  //   based fetch semantics possible.
  // - If not, transports should treat CACHE as a hint and safely ignore it.
  //
  // Current expected transport behavior:
  // - CENTRAL:
  //   understands CACHE as a split wire/store hint around rid.
  // - MESH:
  //   currently ignores CACHE.
  CACHE = 0x20,
}

// /**
//  * Operation Semantics (application-layer intent)
//  */
// export const enum OP {
//   // data plane
//   READ = 1, // Idempotent, no side effects
//   WRITE = 2, // Mutation, side effects
//   // control/coordination plane
//   // /** @unused vec=0으로 하는 게 더 나아보이지만 일단은 이런 방법도 있다 생각하고 reserved */
//   // NOOP = 0xff, // Heartbeat, keep-alive
// }

// ============ Data Plane ============

export type DataFrameBase = {
  tx_id: u32; // transaction identifier (correlates request/response)
  errno: Errno; // OK=0, error<0 (negative errno)

  flags: u32; // FrameFlag bits
  opts: TxOptions; // local-only routing/tx hints; not on wire header

  args: U8Array; // TLV stream (codec-defined terminator)
  body: U8Array | null; // raw; TODO: body will be replaced with SAB pointer handle
};

/**
 * Frame: Message envelope shared across dispatch, runtime, and network boundaries
 * - sel: routing selector (where to send)
 * - vec/cmd: handler selector (what to do)
 * - tx_id: transaction tracking
 */
export type SignalFrame = DataFrameBase & {
  kind: FrameKind.SIGNAL;

  sel: NodeSelector;
  // Correspondent selector (routing metadata).
  //
  // Directional semantics:
  // - REQ (RES=0): selector of the intended correspondent (destination on the wire).
  // - RES (RES=1): selector of the intended correspondent (original requester on the wire).
  //
  // Contract:
  // - Router/gateway may READ sel for routing, but MUST NOT rewrite it.
  // - Caller identity is NOT inferred from sel. If needed, encode it elsewhere (e.g. RID/args/body).

  rid: RID | null;
  vec: Vector;
};

export type PostFrame = DataFrameBase & {
  kind: FrameKind.POST;

  cmd: Command;
};

export type DataFrame = SignalFrame | PostFrame;

/**
 * Top-level Frame union
 * All messages that flow through the bus
 */
export type Frame = ControlFrame | DataFrame;

// Used for transport-level NACK/fault replies and for omitted payloads.
export const EMPTY_ARGS: U8Array = new Uint8Array(0);
