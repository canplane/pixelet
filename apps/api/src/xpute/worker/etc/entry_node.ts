// @pxl/api/xpute/worker/etc/entry_node.ts

/**
 * @unused
 * Worker entry for Node runtime.
 * Resolves the worker uplink, loads app-owned device topology from the manifest,
 * and boots the shared xpute worker runtime.
 */

import { parentPort } from "node:worker_threads";

import { FrameKind, type WorkerArg, type WorkerInit, type WorkerLink } from "@xpute/spec/ipc/frame.ts";
import { type DeviceTopology, WorkerID } from "@xpute/spec/ipc/topology.ts";
import { init_worker } from "@xpute/core/ipc/worker.ts";

import manifest from "@/xpute.manifest.ts";

const IS_WORKER_INIT = (data: unknown): data is WorkerInit => (data as WorkerInit)?.kind === FrameKind.WORKER_INIT;

export const resolve_arg = (): Promise<WorkerArg> => {
  return new Promise((resolve, reject) => {
    const port = parentPort;
    if (!port) {
      // Node worker adapter: parentPort exists only inside worker_threads.
      reject(new Error(`worker entry: missing parent port (not a worker thread?)`));
      return;
    }

    port.postMessage({ kind: FrameKind.WORKER_READY });

    const init_handler = (data: unknown) => {
      port.off("message", init_handler);

      try {
        if (!IS_WORKER_INIT(data)) {
          reject(new Error(`worker entry: unknown worker data: ${JSON.stringify(data)}`));
          return;
        }

        const uplink: WorkerLink = {
          send: (frame) => port.postMessage(frame),
          on: (handler) => {
            port.on("message", handler);
            return () => port.off("message", handler);
          },
        };

        const { kind: _kind, ...rest } = data;
        resolve({ ...rest, uplink });
      } catch (err) {
        reject(err instanceof Error ? err : new Error(String(err)));
      }
    };

    port.on("message", init_handler);
  });
};

function resolve_device_specs(worker_id: WorkerID): DeviceTopology {
  const found = manifest.devices.find(([id]) => id === worker_id);
  return found ? found[1] : [];
}

async function main() {
  const arg: WorkerArg = await resolve_arg();
  const device_specs: DeviceTopology = resolve_device_specs(arg.worker_id);
  await init_worker(arg, device_specs);
}
main().catch((err) => console.error(`worker entry: fatal:`, err));
