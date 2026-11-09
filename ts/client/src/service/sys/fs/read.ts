// @pxl/client/service/sys/fs/read.ts

/** A 204 is a baked page that its version leaves out, and reads as ENOENT. */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { type Fetcher, FileError } from "@xpute/runtime";
import { online } from "@/service/origin/online.ts";
import * as origin from "@/service/origin/route.ts";
import type { NativeFetch } from "@/service/platform/types.ts";

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
    throw transport_error();
  }
  if (res.status === 204 || !res.ok) throw status_error(url, res.status);
  return body;
}

/** A failure of the transport: fetch throws the same TypeError when offline,
 * on a DNS failure, on CORS and on a refused connection, and only the online
 * flag tells offline apart. */
function transport_error(): FileError {
  return new FileError(online() ? Errno.EIO : Errno.ENETUNREACH);
}

/** Why a response of `status` holds no file. */
function status_error(url: string, status: number): FileError {
  if (status === 204) return new FileError(Errno.ENOENT);
  console.warn(`[read] ${url}: HTTP ${status}`);
  // A 404 is the object's failure: its key names nothing. Any other status
  // is the server's failure.
  return new FileError(status === 404 ? Errno.ENXIO : Errno.EACCES, status);
}

export function fetcher_of(native: NativeFetch): Fetcher {
  return {
    cancel: (fd) => native.cancel(fd),
    fetch: (fd, url) =>
      native.fetch(fd, url).then(
        (status) => {
          if (status !== 0) throw status_error(url, status);
        },
        (err) => {
          console.warn(`[read] ${url}: ${String(err)}`);
          throw transport_error();
        },
      ),
  };
}
