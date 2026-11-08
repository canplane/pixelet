// @pxl/client/runtime/open.native.ts

/**
 * A device's core, beside the UI. The runtime runs on the UI's thread and is
 * itself the UI's link. A page's core is opened by open.ts.
 */

import { start } from "@xpute/runtime";
import { follow_refused_files } from "@/service/sys/fs/refusal.ts";
import { declare_source, settled } from "@/service/sys/fs/source.ts";
import { set_online } from "@/service/origin/online.ts";
import { manifest, perf_here } from "@/runtime/start.ts";
import { set_core } from "@/service/core/core.ts";
import { type OnOpen, started, ui } from "@/runtime/ui.ts";
import { fs_changed } from "@/service/core/fs.ts";
import { gpu_present } from "@/service/sys/gpu.ts";
import { source_declare } from "@/service/core/source.ts";
import * as device from "@/service/platform/device.ts";
import * as online from "@/service/platform/online.ts";

/** Resolves once a GPU is present and the source is settled: true when the
 * source was declared, false when it was refused. */
export async function open(on: OnOpen): Promise<boolean> {
  set_core(
    await start(
      manifest(device.handheld),
      ui(on, (files) => follow_refused_files(files, fs_changed)),
    ),
    perf_here,
  );
  started(on);
  online.watch(set_online);
  void declare_source(source_declare);
  await gpu_present();
  return settled();
}
