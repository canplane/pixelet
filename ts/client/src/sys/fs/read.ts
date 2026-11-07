// @pxl/client/sys/fs/read.ts

/**
 * The bus under the backends' reads (tile.ts, asset.ts): an object
 * fetched from the origin, whole, as a file's open answers. 204 is a page its
 * version leaves out, ENOENT; 404 names nothing, a failure like any other.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/runtime";
import { online } from "@/origin/online.ts";
import * as origin from "@/origin/route.ts";

/** The object at `url`, whole. */
export async function get(url: string, signal: AbortSignal): Promise<U8Array> {
  let res: Response;
  let body: U8Array;
  try {
    res = await origin.get(url, signal);
    body = new Uint8Array(await res.arrayBuffer());
  } catch (err) {
    // TypeError covers offline, DNS, CORS and refused alike, so only the
    // browser's own word says offline; the rest is the server's.
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
    // A key that names nothing is the object's failure; any other, the server's.
    throw new FileError(
      res.status === 404 ? Errno.ENXIO : Errno.EACCES,
      res.status,
    );
  }
  return body;
}
