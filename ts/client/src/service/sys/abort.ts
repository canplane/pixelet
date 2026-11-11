// @pxl/client/service/sys/abort.ts

/** The core's last report before it stops, which xpute logs, kept short as a
 * stop code for the UI's panic screen. */

import { strerror } from "@xpute/kit/status/errno.spec.ts";
import type { Abort } from "@xpute/runtime";
import { BUILD_NUMBER } from "@/build.ts";

let code: string | null = null;

/** A published build names its files by number, so its stop code also carries
 * the build number, whose table of paths decodes them. The code is null when
 * the core stopped without a report. */
export function stop_code(): string | null {
  return code;
}

export const onabort = ({ errnum, file, line }: Abort): void => {
  const where = `${file.split("/").pop()}:${line}`;
  code = `${strerror(errnum).split(":")[0]} · ${
    file.startsWith("#") ? `${where} · ${BUILD_NUMBER}` : where
  }`;
};
