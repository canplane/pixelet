// @pxl/client/service/platform/drive.native.ts

/** Tells `line` to the tool that drives a build made for measuring
 * (script/perf/): written by the runtime's writer, at info, which the tool
 * reads from the device's log. */

import { LEVEL_INFO } from "@xpute/host/abi/log/log.enum.ts";
import { log } from "@xpute/expo/xpute.ts";

export function tellDrive(line: string): void {
  log(LEVEL_INFO, "perf", line);
}
