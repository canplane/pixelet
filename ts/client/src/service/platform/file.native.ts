// @pxl/client/service/platform/file.native.ts

/** A device fetches a tile off the JavaScript thread, and decodes it there, and
 * the bytes go straight to the runtime (FileFetch.swift, FileFetch.kt, pxl-core
 * sys/file_body.rs). */

import { PLATFORM } from "./pxl-platform.ts";
import type { NativeFetch } from "./types.ts";

export const native_fetch: NativeFetch | undefined = {
  fetch: (fd, url) => PLATFORM.fileFetch(fd, url),
  cancel: (fd) => PLATFORM.fileCancel(fd),
};
