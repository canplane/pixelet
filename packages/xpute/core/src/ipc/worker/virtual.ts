// @xpute/core/ipc/worker/virtual.ts

import type { Frame, WorkerArg, WorkerLink } from "@xpute/spec/ipc/frame.ts";
import { WorkerID } from "@xpute/spec/ipc/topology.ts";

/**
 * [virtual link factory]
 * creates an in-memory host<->worker link (no real worker thread).
 */
export function create_virtual_link(worker_id: WorkerID) {
  // in-memory message handlers
  let on_host: ((frame: Frame) => void) | undefined; // host-side handler (receives uplink)
  let on_worker: ((frame: Frame) => void) | undefined; // worker-side handler (receives downlink)

  // host side
  const downlink: WorkerLink = {
    send: (frame) => on_worker?.(frame),
    on: (handler) => {
      on_host = handler;
      return () => {
        if (on_host === handler) on_host = undefined;
      };
    },
  };

  // worker side
  const uplink: WorkerLink = {
    send: (frame) => on_host?.(frame),
    on: (handler) => {
      on_worker = handler;
      return () => {
        if (on_worker === handler) on_worker = undefined;
      };
    },
  };

  // worker context
  const arg: WorkerArg = { worker_id, uplink };
  return { downlink, arg };
}
