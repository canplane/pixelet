// @pxl/client/runtime/open.native.ts

/**
 * A device's core, beside the UI: the runtime on the UI's thread is
 * the UI's link (open.ts is a page's).
 */

import { start } from "@xpute/runtime";
import { follow_refused_files } from "@/sys/fs/refusal.ts";
import { declare_source, settled } from "@/sys/fs/source.ts";
import { set_online } from "@/origin/online.ts";
import { manifest, perf_here } from "@/runtime/start.ts";
import { set_core } from "@/port/core.ts";
import { type OnOpen, started, ui } from "@/runtime/ui.ts";
import { fs_changed } from "@/port/fs.ts";
import { gpu_present } from "@/port/gpu.ts";
import { source_declare } from "@/port/source.ts";
import * as device from "@/service/device.ts";
import * as online from "@/service/online.ts";

/** Settled once a GPU has answered and the source is declared, true where
 * it was. */
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
