// @xpute/core/ipc/guest.ts

import { HAS_BITS, SET_BITS, type u32 } from "@xpute/spec/lib/kit/word.ts";
import type { U8Array } from "@xpute/spec/lib/kit/array.ts";
import { Errno } from "@xpute/spec/sys/errno.ts";
import { XputeError } from "@xpute/spec/sys/error.ts";

import { type DataFrame, EMPTY_ARGS, type Frame, FrameFlag, FrameKind, type SignalFrame, type WorkerArg } from "@xpute/spec/ipc/frame.ts";
import type { NetUplink } from "@xpute/spec/ipc/link.ts";
import type { XputeManifest } from "@xpute/spec/ipc/topology.ts";
import type { GuestBootstrapper } from "./worker/contract.ts";

import { TxController } from "./tx.ts";

/** Guest linker (App/Network) */
export async function init_guest(arg: WorkerArg, guest_spec: XputeManifest["guest"], net_spec: XputeManifest["net"]) {
  const { worker_id, uplink } = arg;

  // load guest entry
  const guest = await guest_spec();
  const bootstrap = guest.default as GuestBootstrapper;

  // boot guest & mount driver
  const tx = new TxController(worker_id, uplink);
  const net_uplink: NetUplink = {
    send: (frame: SignalFrame): void => uplink.send(frame),
  };
  const { post, net_gateway, start } = await bootstrap(tx, net_uplink);

  // net_spec-driven network boot is linker responsibility
  await net_gateway.init(net_spec);
  await start?.();

  async function from_uplink(frame: DataFrame): Promise<void> {
    // signal (to network)
    if (frame.kind === FrameKind.SIGNAL) return net_gateway.send(frame);

    // post (to app)
    const flags: u32 = frame.flags;
    if (HAS_BITS(flags, FrameFlag.RES)) return tx.receive(frame); // response frames are owned by TxController.

    let errno: Errno;
    let args;
    let body: U8Array | null;
    try {
      const out = await post.receive(frame);
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
    }
    console.warn(`guest init: unknown frame: ${JSON.stringify(frame)}`);
  });
}
