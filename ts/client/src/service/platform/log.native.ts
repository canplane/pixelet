// @pxl/client/service/platform/log.native.ts

import { XPUTE } from "@xpute/native-runtime";

/** Writes a line to the process's output: stdout on iOS, the log on Android. */
export function log(line: string): void {
  XPUTE.log(line);
}
