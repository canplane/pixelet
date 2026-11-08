// @pxl/client/sys/abort.ts

/** The core's last report before it stops, which it makes through its import
 * SYS_ABORT (pxl-core `sys/io/runtime.rs`). The report is logged whole, and
 * kept short as a stop code for the UI's panic screen. */

import { strerror } from "@xpute/kit/status/errno.spec.ts";
import type { Abort } from "@xpute/runtime";
import { BUILD_NUMBER } from "@/deploy.ts";

let code: string | null = null;

/** The stop code gives the errno's name, the file and the line. A published
 * build names its files by number, so its code also carries the build number,
 * whose table of paths decodes them. The code is null when the core stopped
 * without a report. */
export function stop_code(): string | null {
  return code;
}

export const onabort = ({ errnum, file, line, column, a, b }: Abort): void => {
  console.error(
    `[core] stopped: ${strerror(errnum)} at ${file.replace(
      /^\.+/,
      "",
    )}:${line}:${column} (${a}, ${b})`,
  );
  const where = `${file.split("/").pop()}:${line}`;
  code = `${strerror(errnum).split(":")[0]} · ${
    file.startsWith("#") ? `${where} · ${BUILD_NUMBER}` : where
  }`;
};
