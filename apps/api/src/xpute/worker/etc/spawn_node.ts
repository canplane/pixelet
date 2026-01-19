// @pxl/api/xpute/worker/etc/spawn_node.ts

/**
 * @unused
 * Worker spawner for Node runtime.
 * Spawns the app-owned worker entry artifact emitted by the build.
 *
 * NOTE:
 * This path is resolved at runtime from built output, so storing it in a constant is
 * safe. The browser/Vite worker path is different: its entry URL must stay inline for
 * static worker detection and define replacement.
 */

import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { Worker } from "node:worker_threads";

import { WorkerID } from "@xpute/spec/ipc/topology.ts";
import { type Frame, FrameKind, type WorkerInit, type WorkerLink } from "@xpute/spec/ipc/frame.ts";
import type { WorkerDriver } from "@xpute/core/ipc/worker/contract.ts";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// built worker entry artifact path (resolved relative to this built host module).
const SCRIPT_PATH = path.join(__dirname, "../../worker/entry.js");

export default class NodeDriver implements WorkerDriver {
  async spawn(worker_id: WorkerID): Promise<WorkerLink> {
    const init: WorkerInit = { kind: FrameKind.WORKER_INIT, worker_id };
    const worker = new Worker(SCRIPT_PATH);

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

      const on_frame = (frame: Frame) => dispatch(frame);

      const on_ready = (frame: Frame) => {
        if (frame?.kind !== FrameKind.WORKER_READY) return;

        worker.off("message", on_ready);
        worker.on("message", on_frame);

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

      worker.on("message", on_ready);
    });
  }
}
