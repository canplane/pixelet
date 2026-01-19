// @xpute/spec/ipc/device.ts

import type { i32 } from "@xpute/spec/lib/kit/word.ts";

import type { Command, DataFrame, SignalFrame, Vector } from "./frame.ts";

import { VectorPort } from "@xpute/spec/control/signal.ts";
import { CommandPort } from "@xpute/spec/control/post.ts";

/** App-side Module Execution Environment */
export type XputeContext = {
  post: CommandPort;
};

/** Device-side Module Execution Environment */
export type DeviceEnvironment = {
  readonly signal: VectorPort;
  readonly post: CommandPort;
};

/**
 * DeviceUplink — unified data-plane entry point (host <-> worker/device <-> network)
 *
 * Scope:
 * - `DeviceUplink` is the transport surface used by dispatchers and devices for DataFrame (SIGNAL/POST).
 * - Dispatchers and devices must treat routing/reliability/directionality as opaque.
 *
 * Selector (sel) contract (SIGNAL only):
 * - `sel` is transported and delivered verbatim (no rewriting).
 * - Its meaning is transport-defined; dispatchers and devices treat it as opaque metadata.
 *
 * FrameFlag.RES ownership:
 * - Dispatch layers and devices MUST NOT set/clear FrameFlag.RES.
 * - The runtime/router owns RES for request/response directionality.
 *
 * Reliability (ACKREQ):
 * - ACKREQ=0 (UNACK): runtime MUST NOT produce follow-up frames for this tx_id.
 * - ACKREQ=1 (ACKREQ): runtime MUST produce at least one follow-up frame (may be args=EMPTY_ARGS, body=null).
 *
 * wait() contract:
 * - wait(tx_key) yields the next inbound DataFrame for that tx, or null on timeout EOF.
 * - Type of returned frame (SIGNAL/POST) is determined by runtime routing.
 */
export type DeviceUplink = {
  send(frame: DataFrame): i32; // enqueue/emit a data-plane frame
  wait(tx_key: i32): Promise<DataFrame | null>; // wait for next ACK frame of tx_key (or null on EOF)
};

export type NetUplink = {
  send(frame: SignalFrame): void;
};

/**
 * [Device]
 * 시스템 버스에 매핑된 추상화된 기능 단위입니다.
 * 모든 장치는 이 규격을 통해 신호를 수신하는 '수동적 종착점'이 됩니다.
 */
export type Device = {
  capable_vecs(): Vector[];
  capable_cmds(): Command[];

  receive(frame: DataFrame): Promise<DataFrame>;
};
