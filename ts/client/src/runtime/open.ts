// @pxl/client/runtime/open.ts

/**
 * The shell as the kernel's UI, the same on every platform: the imports it
 * takes, by a table; the GPU's state; the stop. Once started, the shell's
 * sensors and visibility are told the kernel.
 */

import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import type { U8Array } from "@xpute/kit/abi/array.ts";
import { read_abort, type Ui } from "@xpute/runtime";
import { Command as Sys } from "@xpute/runtime/abi/sys.spec.ts";
import {
  Command,
  read_sensor_start,
  read_sensor_stop,
  read_shell_view,
} from "@/abi/cmd.spec.ts";
import { onabort } from "@/sys/abort.ts";
import { kernel, ui as link } from "@/runtime/kernel.ts";
import { GPU_STATE, set_gpu } from "@/port/gpu.ts";
import { SENSOR_LISTENER, sensor_start, sensor_stop } from "@/port/sensor.ts";
import * as snapshot from "@/port/snapshot.ts";
import * as sensor from "@/service/sensor.ts";
import * as visibility from "@/service/visibility.ts";
import type { OnOpen } from "@/service/types.ts";

type Take = (f: BranchCursor, named: U8Array | null) => void;

/** The shell, as the UI the kernel tells. */
export function ui(on: OnOpen): Ui {
  const take = new Map<number, Take>([
    [
      Command.SHELL_SNAPSHOT,
      (_f, named) => {
        snapshot.set_snapshot(named!);
        on.snapshot();
      },
    ],
    [Command.SHELL_VIEW, (f) => snapshot.view_posted(read_shell_view(f))],
    [Command.SENSOR_START, (f) => sensor_start(read_sensor_start(f))],
    [Command.SENSOR_STOP, (f) => sensor_stop(read_sensor_stop(f))],
    [Sys.SYS_ABORT, (f) => onabort(read_abort(f))],
  ]);
  return {
    cmds: [...take.keys()],
    on: (_tag, cmd, f, named) => take.get(cmd)!(f!, named),
    gpu: (state) => set_gpu(GPU_STATE[state]),
    stop: on.terminated,
  };
}

/** The kernel started: the shell's sensors and visibility told it. */
export function started(on: OnOpen): void {
  on.started();
  sensor.listen(SENSOR_LISTENER);
  visibility.watch((visible) => link.visible(visible));
  const { geo, orient } = sensor;
  kernel.sensor_platform(geo.has, orient.has, orient.gated, orient.silence);
  kernel.setting_development(__IS_DEV__);
}
