// @pxl/client/service/runtime.native.ts

/**
 * A device's kernel, beside the shell: the runtime on the shell's thread is
 * the shell's link (service/runtime.ts is a page's).
 */

import { start } from "@xpute/runtime";
import { follow_refused_files } from "@/sys/fs/refusal.ts";
import { declare_source, settled } from "@/sys/fs/source.ts";
import { set_online } from "@/origin/online.ts";
import { manifest, perf_here } from "@/runtime/start.ts";
import { set_kernel } from "@/port/kernel.ts";
import { started, ui } from "@/runtime/open.ts";
import { gpu_present } from "@/port/gpu.ts";
import * as device from "@/service/device.ts";
import * as online from "@/service/online.ts";
import type { OnOpen } from "@/service/types.ts";

/** Settled once a GPU has answered and the source is declared, true where
 * it was. */
export async function open(on: OnOpen): Promise<boolean> {
  set_kernel(
    await start(manifest(device.handheld), ui(on, follow_refused_files)),
    perf_here,
  );
  started(on);
  online.watch(set_online);
  void declare_source();
  await gpu_present();
  return settled();
}
