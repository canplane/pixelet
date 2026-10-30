// @pxl/client/driver/stop.ts

/** The kernel's last report, its system call `abort` (pxl-kernel
 * `sys/stop.rs`), logged whole and kept short as a stop code for the shell's
 * halt. */

import { strerror } from "@xpute/kit/status/errno.spec.ts";
import type { SysHost } from "@xpute/host/ipc/sys.ts";
import { BUILD_NUMBER } from "@/deploy.ts";

let code: string | null = null;

/** The stop code: the errno's name and the file and line, a published build's
 * file a number its paths decode by. None where it stopped without one. */
export function stop_code(): string | null {
  return code;
}

export const abort: SysHost["abort"] = ({
  errnum,
  file,
  line,
  column,
  a,
  b,
}) => {
  console.error(
    `[kernel] stopped: ${strerror(errnum)} at ${file.replace(
      /^\.+/,
      "",
    )}:${line}:${column} (${a}, ${b})`,
  );
  const where = `${file.split("/").pop()}:${line}`;
  code = `${strerror(errnum).split(":")[0]} · ${
    file.startsWith("#") ? `${where} · ${BUILD_NUMBER}` : where
  }`;
};
