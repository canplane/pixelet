// @pxl/client/runtime/open.native.ts

/** The runtime runs on its own thread, and the UI reaches it through a link
 * (@xpute/runtime start.native.ts). */

import { Command as GpuCommand } from "@xpute/gpu/abi.spec.ts";
import { gpu_protocol } from "@xpute/gpu/gpu.ts";
import { start } from "@xpute/runtime";
import { Major } from "@/abi/cmd.spec.ts";
import { follow_refused_files } from "@/service/sys/fs/refusal.ts";
import { declare_source, settled } from "@/service/sys/fs/source.ts";
import { set_online } from "@/service/origin/online.ts";
import { manifest, perf_here } from "@/runtime/start.ts";
import { set_core } from "@/service/core/core.ts";
import { type OnOpen, started, ui } from "@/runtime/ui.ts";
import { fs_changed } from "@/service/core/fs.ts";
import { GPU_STATE, gpu_present, set_gpu } from "@/service/sys/gpu.ts";
import { gpu } from "@/service/platform/gpu.ts";
import { source_declare } from "@/service/core/source.ts";
import * as device from "@/service/platform/device.ts";
import * as online from "@/service/platform/online.ts";

/** Resolves once a GPU is present and the source is settled: true when the
 * source was declared, false when it was refused. */
export async function open(on: OnOpen): Promise<boolean> {
  const link = await start(
    manifest(device.handheld),
    ui(on, (files) => follow_refused_files(files, fs_changed)),
  );
  set_core(link, perf_here);
  gpu.watch((state) => set_gpu(GPU_STATE[state]));
  gpu.grant({
    ready: () => gpu_protocol(link, Major.GPU).send(GpuCommand.GPU_READY),
  });
  started(on);
  online.watch(set_online);
  void declare_source(source_declare);
  await gpu_present();
  return settled();
}
