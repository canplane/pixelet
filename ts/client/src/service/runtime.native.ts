// @pxl/client/service/runtime.native.ts

/**
 * A device's runtime, on the shell's JavaScript thread beside the kernel's own,
 * which shares its memory (service/runtime.ts is a page's): the kernel's
 * commands sent to it directly, the snapshot read where the kernel wrote it.
 */

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import { shell_imports, type ShellImports } from "@/abi/cmd.spec.ts";
import { SOURCE_DECLARED } from "@/abi/state.ts";
import { declare_source, source_state } from "@/sys/fs/source.ts";
import { follow_refused_files } from "@/sys/fs/refusal.ts";
import { onabort } from "@/sys/abort.ts";
import { set_online } from "@/origin/online.ts";
import { kernel, set_kernel, set_runtime } from "@/runtime/kernel.ts";
import { start } from "@/runtime/start.ts";
import { SENSOR_LISTENER, sensor_start, sensor_stop } from "@/port/sensor.ts";
import { text_layout, text_raster } from "@/port/text.ts";
import { GPU_STATE, gpu_present, set_gpu } from "@/port/gpu.ts";
import * as snapshot from "@/port/snapshot.ts";
import { answer, type PerfRead } from "@/port/perf.ts";
import { gpu } from "@/service/gpu.ts";
import * as online from "@/service/online.ts";
import * as sensor from "@/service/sensor.ts";
import * as visibility from "@/service/visibility.ts";
import type { OnOpen } from "@/service/types.ts";

/** The runtime started and the kernel's commands bound to it; settled once a
 * GPU has answered and the world's source is declared, true where it was. */
export async function open(on: OnOpen): Promise<boolean> {
  const SHELL: ShellImports = {
    shell_snapshot: (e) => {
      snapshot.set_snapshot(new Uint8Array(runtime.core.mem, e.at, e.len));
      follow_refused_files(snapshot.current().refused_files);
      on.snapshot();
    },
    shell_view: snapshot.view_posted,
    sensor_start,
    sensor_stop,
    text_layout,
    text_raster,
  };
  const { runtime, pacing } = await start(
    shell_imports(SHELL),
    onabort,
    on.terminated,
  );
  set_runtime(runtime);
  set_kernel(runtime, {
    resume: () => runtime.resume(),
    pause: () => runtime.pause(),
    wake: () => runtime.wake(),
    input: () => pacing.input(performance.now()),
    visible: (visible) => runtime.visible(visible),
    fits: (encode) => {
      const pkt = encode(runtime.doorbell);
      return pkt === null || runtime.doorbell.fits(pkt);
    },
    cmd: (cmd, ...args) => runtime.doorbell.cmd(cmd, ...args),
    // A device's views are bound by the native module, not lent a canvas.
    bind: () => {
      throw new InvariantError(Errno.ENOTSUP);
    },
    unbind: () => {
      throw new InvariantError(Errno.ENOTSUP);
    },
    perf: (what, args) => Promise.resolve(answer(what as PerfRead, args)),
  });
  on.started();
  sensor.listen(SENSOR_LISTENER);
  gpu.watch((state) => set_gpu(GPU_STATE[state]));
  online.watch(set_online);
  // The runtime tells the kernel it is hidden or shown, and holds the turns
  // while it is hidden.
  visibility.watch((visible) => runtime.visible(visible));
  // What the platform's sensors are, and whether this is a development
  // build, told after the runtime's attach, which the kernel starts on.
  const { geo, orient } = sensor;
  kernel.sensor_platform(geo.has, orient.has, orient.gated, orient.silence);
  kernel.setting_development(__IS_DEV__);
  const declared = declare_source();
  await gpu_present();
  await declared;
  return source_state() === SOURCE_DECLARED;
}
