// @pxl/client/service/sys/abort.ts

/** The core's report as it stops, which xpute's runtime writes
 * (@xpute/runtime-web worker.ts `tell_stop`), kept short as a stop code for
 * the UI's panic screen. */

import { strerror } from "@xpute/kit/status/error.ts";
import type { UiStop } from "@xpute/host";
import { BUILD_NUMBER } from "@/build.ts";

let code: string | null = null;

/** A published build names its files by number, so its stop code also carries
 * the build number, whose table of paths decodes them. The code is null when
 * the core stopped without a report. */
export function stopCode(): string | null {
  return code;
}

export const stopped = ({ errnum, file, line }: UiStop): void => {
  const where = `${file.split("/").pop()}:${line}`;
  code = `${strerror(errnum).split(":")[0]} · ${
    file.startsWith("#") ? `${where} · ${BUILD_NUMBER}` : where
  }`;
};
