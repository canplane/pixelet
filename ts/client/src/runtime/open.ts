// @pxl/client/runtime/open.ts

/** A page's core runs in a worker of its own, and the UI's link to it is
 * xpute's, across the worker's port. */

import { gpu_connect } from "@xpute/gpu/web.ts";
import { start } from "@xpute/runtime";
import { MANIFEST } from "@/abi/manifest.spec.ts";
import { keep_said } from "@xpute/runtime/say.ts";
import { mirror_source, settled } from "@/service/sys/fs/source.ts";
import { is_storage_key } from "@/service/sys/fs/storage.ts";
import { set_online } from "@/service/origin/online.ts";
import { set_core } from "@/service/core/core.ts";
import type { ToCore, ToUi } from "@/runtime/page.ts";
import { type OnOpen, started, ui } from "@/runtime/ui.ts";
import { GPU_STATE, gpu_present, set_gpu } from "@/service/sys/gpu.ts";
import { set_canvas_binder } from "@/service/platform/gpu.ts";
import * as device from "@/service/platform/device.ts";
import * as online from "@/service/platform/online.ts";
import * as storage from "@/service/platform/storage.ts";

/** Resolves once a GPU is present and the source is settled: true when the
 * source was declared, false when it was refused. */
export async function open(on: OnOpen): Promise<boolean> {
  // A build made for measuring keeps the page's lines, the runtime's among
  // them, for the tool that drives it (script/perf/).
  if (__PERF_DRIVE__) keep_said();
  // Metro bundles the worker through this call, and through no form of
  // `new Worker` that tree shaking also compiles (metro.config.ts).
  const worker = new Worker(require.unstable_resolveWorker("./worker.ts"), {
    type: "module",
  });
  const ask = (m: ToCore) => worker.postMessage(m);
  worker.onmessage = ({ data: m }: MessageEvent<ToUi>) => {
    switch (m.kind) {
      case "source":
        return mirror_source(m.state, m.decl);
      case "keep":
        return storage.set(m.key, m.val);
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
  const { port1, port2 } = new MessageChannel();
  set_canvas_binder(gpu_connect(port1, (state) => set_gpu(GPU_STATE[state])));
  worker.postMessage({ kind: "gpu", port: port2 } satisfies ToCore, [port2]);
  const link = await start(
    MANIFEST,
    ui(on, (files) => ask({ kind: "refused", files })),
    () => worker,
  );
  set_core(link);
  started(on);
  online.watch(set_online);
  await gpu_present();
  return settled();
}
