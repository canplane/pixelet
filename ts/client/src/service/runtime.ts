// @pxl/client/service/runtime.ts

/**
 * A page's kernel, in a worker of its own (runtime/worker.ts): the shell's
 * link is xpute's, across the worker's port; what a worker cannot read of the
 * page and the files the kernel refused go to it, and what it keeps and reads
 * comes back (runtime/page.ts).
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { start } from "@xpute/runtime";
import { mirror_source, settled } from "@/sys/fs/source.ts";
import { is_kept } from "@/sys/fs/storage.ts";
import { set_online } from "@/origin/online.ts";
import { set_kernel } from "@/runtime/kernel.ts";
import type { ToKernel, ToShell } from "@/runtime/page.ts";
import { manifest } from "@/runtime/start.ts";
import { started, ui } from "@/runtime/open.ts";
import { gpu_present } from "@/port/gpu.ts";
import * as device from "@/service/device.ts";
import * as online from "@/service/online.ts";
import * as storage from "@/service/storage.ts";
import type { OnOpen } from "@/service/types.ts";

/** Settled once a GPU has answered and the source is declared, true where
 * it was. */
export async function open(on: OnOpen): Promise<boolean> {
  // Against the page, not `import.meta.url`: Expo's stand-in for that comes
  // with `expo`, which the boot runs ahead of (index.ts).
  const worker = new Worker(new URL("../runtime/worker.ts", location.href), {
    type: "module",
  });
  const ask = (m: ToKernel) => worker.postMessage(m);
  const reads = new Map<u32, (value: unknown) => void>();
  let next = 0;
  worker.onmessage = ({ data: m }: MessageEvent<ToShell>) => {
    switch (m.kind) {
      case "source":
        return mirror_source(m.state, m.declaration);
      case "keep":
        return storage.set(m.key, m.value);
      case "perf":
        reads.get(m.id)?.(m.value);
        return void reads.delete(m.id);
    }
  };
  ask({
    kind: "page",
    keys: storage
      .keys()
      .filter(is_kept)
      .map((key) => [key, storage.get(key)!]),
    handheld: device.handheld,
  });
  const link = await start(
    manifest(device.handheld),
    ui(on, (files) => ask({ kind: "refused", files })),
    () => worker,
  );
  set_kernel(
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
