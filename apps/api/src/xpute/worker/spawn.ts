// @pxl/api/xpute/worker/spawn.ts

/**
 * Worker spawner for Deno runtime.
 * Spawns the app-owned worker entry artifact emitted by the build.
 *
 * NOTE:
 * Unlike the browser/Vite path, this runtime loads an already-built worker artifact.
 * Keeping the worker script path in a constant is safe here because this is runtime
 * path resolution, not Vite worker-entry detection.
 */

import { WorkerID } from "@xpute/spec/ipc/topology.ts";
import { type Frame, FrameKind, type WorkerInit, type WorkerLink } from "@xpute/spec/ipc/frame.ts";
import type { WorkerDriver } from "@xpute/core/ipc/worker/contract.ts";

// built worker entry artifact path (resolved relative to this built host module).
const SCRIPT_PATH = new URL("../worker.js", import.meta.url);

export default class DenoDriver implements WorkerDriver {
  async spawn(worker_id: WorkerID): Promise<WorkerLink> {
    const init: WorkerInit = { kind: FrameKind.WORKER_INIT, worker_id };
    const worker = new Worker(SCRIPT_PATH, { type: "module" });

    return await new Promise((resolve, reject) => {
      const handlers: ((frame: Frame) => void)[] = [];
      const inbox: Frame[] = [];

      const dispatch = (frame: Frame) => {
        if (frame?.kind === FrameKind.WORKER_READY) return;
        if (handlers.length === 0) {
          inbox.push(frame);
          return;
        }
        for (const h of handlers) h(frame);
      };

      const on_frame = (e: MessageEvent) => dispatch(e.data as Frame);

      const on_ready = (e: MessageEvent) => {
        const frame = e.data as Frame;
        if (frame?.kind !== FrameKind.WORKER_READY) return;

        worker.removeEventListener("message", on_ready);
        worker.addEventListener("message", on_frame);

        // handshake: wait for WORKER_READY, then send WORKER_INIT.
        worker.postMessage(init);

        const downlink: WorkerLink = {
          send: (frame) => worker.postMessage(frame),
          on: (handler) => {
            const first = handlers.length === 0;
            handlers.push(handler);
            if (first && inbox.length) {
              for (const f of inbox) handler(f);
              inbox.length = 0;
            }
            return () => {
              const i = handlers.indexOf(handler);
              if (i >= 0) handlers.splice(i, 1);
            };
          },
        };

        resolve(downlink);
      };

      worker.addEventListener("message", on_ready);
    });
  }
}
