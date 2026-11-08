// @pxl/client/service/sys/fs/read.ts

/**
 * The read under the backends that fetch (tile.ts, asset.ts). It fetches an
 * object whole from the origin and answers as a file's open does. A 204 is a
 * baked page that its version leaves out, and reads as ENOENT. A 404 means
 * the key names nothing, which is a failure like any other.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/runtime";
import { online } from "@/service/origin/online.ts";
import * as origin from "@/service/origin/route.ts";

export async function get(url: string, signal: AbortSignal): Promise<U8Array> {
  let res: Response;
  let body: U8Array;
  try {
    res = await origin.get(url, signal);
    body = new Uint8Array(await res.arrayBuffer());
  } catch (err) {
    // fetch throws the same TypeError when offline, on a DNS failure, on CORS
    // and on a refused connection. Only the browser's online flag tells
    // offline apart; every other failure is counted as the server's.
    if (!signal.aborted) {
      console.warn(
        `[read] ${url}: ${
          err instanceof Error ? `${err.name}: ${err.message}` : String(err)
        }`,
      );
    }
    throw new FileError(online() ? Errno.EIO : Errno.ENETUNREACH);
  }
  if (res.status === 204) throw new FileError(Errno.ENOENT);
  if (!res.ok) {
    console.warn(`[read] ${url}: HTTP ${res.status}`);
    // A 404 is the object's failure: its key names nothing. Any other status
    // is the server's failure.
    throw new FileError(
      res.status === 404 ? Errno.ENXIO : Errno.EACCES,
      res.status,
    );
  }
  return body;
}
