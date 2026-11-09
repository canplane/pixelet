// @pxl/client/runtime/open.ts

/** A page's core runs in a worker of its own, and the UI's link to it is
 * xpute's, across the worker's port. */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { start } from "@xpute/runtime";
import { mirror_source, settled } from "@/service/sys/fs/source.ts";
import { is_storage_key } from "@/service/sys/fs/storage.ts";
import { set_online } from "@/service/origin/online.ts";
import { set_core } from "@/service/core/core.ts";
import type { ToCore, ToUi } from "@/runtime/page.ts";
import { manifest } from "@/runtime/start.ts";
import { type OnOpen, started, ui } from "@/runtime/ui.ts";
import { gpu_present } from "@/service/sys/gpu.ts";
import * as device from "@/service/platform/device.ts";
import * as online from "@/service/platform/online.ts";
import * as storage from "@/service/platform/storage.ts";

/** Resolves once a GPU is present and the source is settled: true when the
 * source was declared, false when it was refused. */
export async function open(on: OnOpen): Promise<boolean> {
  // The worker's URL is resolved against the page rather than
  // `import.meta.url`. Expo's stand-in for `import.meta` comes with `expo`,
  // and the boot runs before `expo` is imported (index.ts).
  const worker = new Worker(new URL("./worker.ts", location.href), {
    type: "module",
  });
  const ask = (m: ToCore) => worker.postMessage(m);
  const reads = new Map<u32, (val: unknown) => void>();
  let next = 0;
  worker.onmessage = ({ data: m }: MessageEvent<ToUi>) => {
    switch (m.kind) {
      case "source":
        return mirror_source(m.state, m.decl);
      case "keep":
        return storage.set(m.key, m.val);
      case "perf":
        reads.get(m.id)?.(m.val);
        return void reads.delete(m.id);
    }
  };
  ask({
    kind: "page",
    keys: storage
      .keys()
      .filter(is_storage_key)
      .map((key) => [key, storage.get(key)!]),
    handheld: device.handheld,
  });
  const link = await start(
    manifest(device.handheld),
    ui(on, (files) => ask({ kind: "refused", files })),
    () => worker,
  );
  set_core(
    link,
    (what, args) =>
      new Promise((done) => {
        const id = (next = (next + 1) >>> 0 || 1);
        reads.set(id, done);
        ask({ kind: "perf", id, what, args });
      }),
  );
  started(on);
  online.watch(set_online);
  await gpu_present();
  return settled();
}
