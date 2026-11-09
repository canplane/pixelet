// @pxl/client/service/platform/say.native.ts

import { XPUTE } from "@xpute/native";

/** Writes a line to the process's output: stdout on iOS, the log on Android. */
export function say(line: string): void {
  XPUTE.say(line);
}
