// @pxl/client/runtime/ui.ts

import { abort_of, type Ui } from "@xpute/runtime";
import type { RefusedFile } from "@/abi/cmd.spec.ts";
import { Command as Sys, sys_imports } from "@xpute/runtime/abi/sys.spec.ts";
import { Command, ui_imports } from "@/abi/cmd.spec.ts";
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

/** `refused` takes the files a snapshot lists as refused to where the files
 * are kept. */
export function ui(
  on: OnOpen,
  refused: (files: readonly RefusedFile[]) => void,
): Ui {
  return {
    protocols: [
      ui_imports({
        [Command.UI_SNAPSHOT]: (_e, named) => {
          snapshot.set_snapshot(named);
          refused(snapshot.cur().refused_files);
          on.snapshot();
        },
        [Command.UI_VIEW]: snapshot.view_posted,
        [Command.SENSOR_START]: sensor_start,
        [Command.SENSOR_STOP]: sensor_stop,
      }),
      sys_imports({
        [Sys.SYS_ABORT]: (e) => onabort(abort_of(e)),
        // The core's pace is the runtime's (sched/policy.ts).
        [Sys.SYS_PACE]: () => {},
      }),
    ],
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
  core.sensor_platform(geo.has, orient.has, orient.gated, orient.silence);
  core.setting_dev(__IS_DEV__ || __PERF_DRIVE__);
}
