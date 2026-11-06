// @pxl/client/sys/abort.ts

/** The kernel's last report, its import SYS_ABORT (pxl-kernel
 * `sys/io/runtime.rs`), logged whole and kept short as a stop code for the shell's
 * panic. */

import { strerror } from "@xpute/kit/status/errno.spec.ts";
import type { Abort } from "@xpute/runtime";
import { BUILD_NUMBER } from "@/deploy.ts";

let code: string | null = null;

/** The stop code: the errno's name and the file and line, a published build's
 * file a number its paths decode by. None where it stopped without one. */
export function stop_code(): string | null {
  return code;
}

export const onabort = ({ errnum, file, line, column, a, b }: Abort): void => {
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
