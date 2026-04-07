// @xpute/core/ipc/worker.ts

/**
 * worker.ts — Runtime router for local workers
 *
 * This module owns bus-level routing contracts that devices and dispatch layers must not touch:
 *
 * 1) FrameFlag.RES ownership (REQ/RES bit)
 *    - Devices / VectorPort MUST NOT set/clear FrameFlag.RES.
 *    - The runtime/router sets RES=1 only when emitting a response frame.
 *    - Requests are delivered to devices with RES=0.
 *
 * 2) FrameFlag.ACKREQ is behavioral (not a hint)
 *    - ACKREQ=0 (UNACK):
 *        - Fire-and-forget semantics.
 *        - The runtime MUST NOT emit a response frame.
 *        - Any device return value is discarded at the router boundary.
 *    - ACKREQ=1 (ACKREQ):
 *        - The runtime MUST emit exactly one response frame per inbound request.
 *        - Response args/body may be EMPTY/null; errno encodes failure if any.
 *
 * 3) NodeSelector `sel` contract (correspondent selector)
 *    - sel is routing metadata: "who this frame is addressed to / associated with".
 *    - Directional semantics:
 *        - REQ (RES=0): sel = correspondent selector chosen by requester (wire dst).
 *        - RES (RES=1): sel = correspondent selector chosen by responder (wire dst).
 *    - Router/gateway may READ sel for routing decisions, but MUST NOT rewrite it.
 *    - Caller identity is NOT derived from sel. If identity is required, encode it in RID/args/body.
 *
 * 4) Error mapping
 *    - Device exceptions are mapped to errno and returned only if ACKREQ=1.
 *    - For ACKREQ=0, exceptions do not generate responses (best-effort).
 *
 * 5) Args/body decode failures (marshal boundary)
 *    - Any args/body decode error (e.g., PacketMarshal length mismatch, truncated TLV)
 *      is treated as message corruption at this boundary (not an application error).
 *    - Mapping policy:
 *        - ACKREQ=1: respond with errno=EBADMSG (or EILSEQ) and args=EMPTY_ARGS, body=null.
 *        - ACKREQ=0: drop silently (no response).
 *
 * Rationale:
 * - Decode failures are not "application errors" and must not escape as JS exceptions
 *   across the transport boundary. They are mapped to errno by the runtime.
 */

import { HAS_BITS, SET_BITS, type u32 } from "@xpute/spec/lib/kit/word.ts";
import type { U8Array } from "@xpute/spec/lib/kit/array.ts";
import { Errno } from "@xpute/spec/sys/errno.ts";
import { XputeError } from "@xpute/spec/sys/error.ts";

import type { Device } from "@xpute/spec/ipc/link.ts";
import { type Command, type DataFrame, EMPTY_ARGS, type Frame, FrameFlag, FrameKind, type Vector, type WorkerArg } from "@xpute/spec/ipc/frame.ts";
import { DevicePort, type DeviceTopology, type XputeManifest } from "@xpute/spec/ipc/topology.ts";

import { load_device } from "./worker/device_loader.ts";
import { TxController } from "./tx.ts";

/** Device linker */
export async function init_worker(arg: WorkerArg, device_specs: DeviceTopology, wasm_loader: XputeManifest["wasm"]): Promise<void> {
  const { worker_id, uplink } = arg;

  // devices
  const tx = new TxController(worker_id, uplink);
  const device_entries: [DevicePort, Device][] = await Promise.all(
    device_specs.map(async ([port, device_spec]) => {
      const device = await load_device(port, device_spec, tx, wasm_loader);
      return [port, device];
    }),
  );
  const devices: Map<DevicePort, Device> = new Map(device_entries);

  // link handshake
  for (const [port, device] of devices) {
    uplink.send({
      kind: FrameKind.LINK_INIT_SYN,
      port,
      capable_vecs: device.capable_vecs(),
      capable_cmds: device.capable_cmds(),
    });
  }

  // switch tables
  const vec_table = new Map<Vector, DevicePort>();
  const cmd_table = new Map<Command, DevicePort>();

  async function from_uplink(frame: DataFrame): Promise<void> {
    const flags: u32 = frame.flags;
    if (HAS_BITS(flags, FrameFlag.RES)) return tx.receive(frame); // response frames are owned by TxController.

    // route -> device
    let port: DevicePort | undefined;
    switch (frame.kind) {
      case FrameKind.SIGNAL:
        port = vec_table.get(frame.vec);
        break;
      case FrameKind.POST:
        port = cmd_table.get(frame.cmd);
        break;

      default:
        return console.warn(`worker init: unknown frame: ${JSON.stringify(frame)}`);
    }

    let errno: Errno;
    let args;
    let body: U8Array | null;
    try {
      if (!port) throw new XputeError(Errno.ENOENT);
      const dev: Device | undefined = devices.get(port);
      if (!dev) throw new XputeError(Errno.ENETUNREACH);

      const out = await dev.receive(frame);
      errno = out.errno;
      args = out.args;
      body = out.body;
    } catch (err) {
      errno = err instanceof XputeError ? err.errno : Errno.EBADMSG;
      args = EMPTY_ARGS;
      body = null;
    }

    // ACKREQ is behavioral: if not set, do not emit responses.
    if (HAS_BITS(flags, FrameFlag.ACKREQ)) {
      uplink.send({
        ...frame, // echo invariant: preserve non-payload fields
        flags: SET_BITS(flags, FrameFlag.RES),
        errno,
        args,
        body,
      });
    }
  }

  uplink.on(async (frame: Frame): Promise<void> => {
    switch (frame.kind) {
      case FrameKind.SIGNAL:
      case FrameKind.POST:
        return await from_uplink(frame);

      case FrameKind.LINK_INIT_ACK:
        frame.vec_table.forEach(([vec, port]) => vec_table.set(vec, port));
        frame.cmd_table.forEach(([cmd, port]) => cmd_table.set(cmd, port));
        return;
    }
    console.warn(`init_worker: unknown frame: ${JSON.stringify(frame)}`);
  });
}
