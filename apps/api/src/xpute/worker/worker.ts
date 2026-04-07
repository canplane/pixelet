// @pxl/api/xpute/worker/worker.ts

/**
 * Worker entry for Deno runtime.
 * Resolves the worker uplink, loads app-owned device topology from the manifest,
 * and boots the shared xpute worker runtime.
 */

import { FrameKind, type WorkerArg, type WorkerInit, type WorkerLink } from "@xpute/spec/ipc/frame.ts";
import { type DeviceTopology, WorkerID } from "@xpute/spec/ipc/topology.ts";
import { init_worker } from "@xpute/core/ipc/worker.ts";

import manifest from "@/xpute.manifest.ts";

const IS_WORKER_INIT = (data: unknown): data is WorkerInit => (data as WorkerInit)?.kind === FrameKind.WORKER_INIT;

type MessageLink = {
  postMessage(payload: unknown): void;
  addEventListener(type: "message", listener: EventListener): void;
  removeEventListener(type: "message", listener: EventListener): void;
};

export const resolve_arg = (): Promise<WorkerArg> => {
  return new Promise((resolve, reject) => {
    // Deno worker adapter: reject when a window global is present.
    if (typeof (globalThis as any).window !== "undefined") {
      reject(new Error("worker entry: not a worker context"));
      return;
    }

    const g = globalThis as unknown as Partial<MessageLink>;
    if (!g.postMessage || !g.addEventListener || !g.removeEventListener) {
      reject(new Error(`worker entry: worker context missing message link apis`));
      return;
    }

    const uplink: WorkerLink = {
      send: (frame) => {
        const payload = frame;
        g.postMessage!(payload);
      },
      on: (handler) => {
        const h = ((e: Event) => handler((e as MessageEvent).data)) as EventListener;
        g.addEventListener!("message", h);
        return () => g.removeEventListener!("message", h);
      },
    };

    uplink.send({ kind: FrameKind.WORKER_READY });

    const init_handler = ((e: Event) => {
      g.removeEventListener!("message", init_handler);

      try {
        const data = (e as MessageEvent).data as unknown;
        if (!IS_WORKER_INIT(data)) {
          reject(new Error(`worker entry: unknown worker data: ${JSON.stringify(data)}`));
          return;
        }

        const { kind: _kind, ...rest } = data;
        resolve({ ...rest, uplink });
      } catch (err) {
        reject(err instanceof Error ? err : new Error(String(err)));
      }
    }) as EventListener;

    g.addEventListener("message", init_handler);
  });
};

function resolve_device_specs(worker_id: WorkerID): DeviceTopology {
  const found = manifest.devices.find(([id]) => id === worker_id);
  return found ? found[1] : [];
}

async function main() {
  const arg: WorkerArg = await resolve_arg();
  const device_specs: DeviceTopology = resolve_device_specs(arg.worker_id);

  await init_worker(arg, device_specs, manifest.wasm);
}
main().catch((err) => console.error(`worker entry: fatal:`, err));
