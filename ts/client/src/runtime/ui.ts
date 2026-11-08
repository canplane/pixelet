// @pxl/client/runtime/ui.ts

/**
 * The UI that the core talks to, the same on every platform. It takes the
 * core's imports through a table, and hears the GPU's state and the core's
 * stop. Once the core has started, the UI tells it about the sensors and the
 * visibility.
 */

import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import type { U8Array } from "@xpute/kit/abi/array.ts";
import { read_abort, type Ui } from "@xpute/runtime";
import type { RefusedFile } from "@/abi/cmd.spec.ts";
import { Command as Sys } from "@xpute/runtime/abi/sys.spec.ts";
import {
  Command,
  read_sensor_start,
  read_sensor_stop,
  read_ui_view,
} from "@/abi/cmd.spec.ts";
import { onabort } from "@/service/sys/abort.ts";
import { core, ui as link } from "@/service/core/core.ts";
import { GPU_STATE, set_gpu } from "@/service/sys/gpu.ts";
import {
  SENSOR_LISTENER,
  sensor_start,
  sensor_stop,
} from "@/service/core/sensor.ts";
import * as snapshot from "@/service/core/snapshot.ts";
import * as sensor from "@/service/platform/sensor.ts";
import { listen } from "@/service/platform/sensor-listener.ts";
import * as visibility from "@/service/platform/visibility.ts";

/** What the client's boot hears as the runtime opens (boot.ts). */
export interface OnOpen {
  /** The runtime started, and the core's commands reach it. */
  started(): void;
  /** A snapshot came. */
  snapshot(): void;
  /** The core stopped. */
  terminated(): void;
}

type Take = (f: BranchCursor, named: U8Array | null) => void;

/** The UI the core talks to. Only this reads the snapshot, and `refused`
 * takes the refused files it lists to where the files are kept. */
export function ui(
  on: OnOpen,
  refused: (files: readonly RefusedFile[]) => void,
): Ui {
  const take = new Map<number, Take>([
    [
      Command.UI_SNAPSHOT,
      (_f, named) => {
        snapshot.set_snapshot(named!);
        refused(snapshot.current().refused_files);
        on.snapshot();
      },
    ],
    [Command.UI_VIEW, (f) => snapshot.view_posted(read_ui_view(f))],
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

/** Called when the core has started. It tells the core about the sensors and
 * the visibility, and keeps it told from then on. */
export function started(on: OnOpen): void {
  on.started();
  listen(SENSOR_LISTENER);
  visibility.watch((visible) => link.visible(visible));
  const { geo, orient } = sensor;
  core.sensor_platform(geo.has, orient.has, orient.gated, orient.silence);
  core.setting_development(__IS_DEV__);
}
