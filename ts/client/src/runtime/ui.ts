// @pxl/client/runtime/ui.ts

import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import type { U8Array } from "@xpute/kit/abi/array.ts";
import { read_abort, type Ui } from "@xpute/runtime";
import type { RefusedFile } from "@/abi/cmd.spec.ts";
import { Command as Sys } from "@xpute/runtime/abi/sys.spec.ts";
import { Command, read } from "@/abi/cmd.spec.ts";
import { onabort } from "@/service/sys/abort.ts";
import { core, ui as link } from "@/service/core/core.ts";
import {
  SENSOR_LISTENER,
  sensor_start,
  sensor_stop,
} from "@/service/core/sensor.ts";
import * as snapshot from "@/service/core/snapshot.ts";
import * as sensor from "@/service/platform/sensor.ts";
import { listen } from "@/service/platform/sensor-listener.ts";
import * as visibility from "@/service/platform/visibility.ts";

export interface OnOpen {
  /** The runtime started, and the core's commands reach it. */
  started(): void;
  snapshot(): void;
  terminated(): void;
}

type Take = (f: BranchCursor, named: U8Array | null) => void;

/** `refused` takes the files a snapshot lists as refused to where the files
 * are kept. */
export function ui(
  on: OnOpen,
  refused: (files: readonly RefusedFile[]) => void,
): Ui {
  const take = new Map<number, Take>([
    [
      Command.UI_SNAPSHOT,
      (_f, named) => {
        snapshot.set_snapshot(named!);
        refused(snapshot.cur().refused_files);
        on.snapshot();
      },
    ],
    [Command.UI_VIEW, (f) => snapshot.view_posted(read[Command.UI_VIEW](f))],
    [Command.SENSOR_START, (f) => sensor_start(read[Command.SENSOR_START](f))],
    [Command.SENSOR_STOP, (f) => sensor_stop(read[Command.SENSOR_STOP](f))],
    [Sys.SYS_ABORT, (f) => onabort(read_abort(f))],
  ]);
  return {
    cmds: [...take.keys()],
    on: (_tag, cmd, f, named) => take.get(cmd)!(f!, named),
    stop: on.terminated,
  };
}

/** A caller calls this once the core has started and its commands are bound.
 * From here on the core is told about the sensors and the visibility. */
export function started(on: OnOpen): void {
  on.started();
  listen(SENSOR_LISTENER);
  visibility.watch((visible) => link.visible(visible));
  const { geo, orient } = sensor;
  core.send(
    Command.SENSOR_PLATFORM,
    geo.has,
    orient.has,
    orient.gated,
    orient.silence,
  );
  core.send(Command.SETTING_DEV, __IS_DEV__ || __PERF_DRIVE__);
}
