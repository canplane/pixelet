// @pxl/web/xpute/worker/spawn.ts

/**
 * Worker spawner for Browser runtime.
 * Spawns the app-owned worker entry source module through Vite.
 *
 * IMPORTANT:
 * Keep the worker entry path inline in `new Worker(new URL("./entry.ts", import.meta.url), ...)`.
 * Do not hoist the path into a variable/constant.
 *
 * Vite statically detects worker entries from this inline pattern. If the path is
 * extracted, worker bundling and compile-time define replacement (for example
 * `__ENV__`, `__IS_DEV__`) can break inside the worker graph.
 */

import { WorkerID } from "@xpute/spec/ipc/topology.ts";
import { type Frame, FrameKind, type WorkerInit, type WorkerLink } from "@xpute/spec/ipc/frame.ts";
import type { WorkerDriver } from "@xpute/core/ipc/worker/contract.ts";

export default class BrowserDriver implements WorkerDriver {
  async spawn(worker_id: WorkerID): Promise<WorkerLink> {
    const init: WorkerInit = { kind: FrameKind.WORKER_INIT, worker_id };
    // NOTE: keep this URL inline so Vite can statically detect the worker entry.
    const worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });

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
