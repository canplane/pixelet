// @pxl/client/namespace/read.ts

/**
 * The bus under the backends' reads (tile.ts, asset.ts): an object
 * fetched from the origin, whole, as a file's open answers. 204 is a page its
 * version leaves out, ENOENT; 404 names nothing, a failure like any other.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/host/ipc/sys.ts";
import { online } from "@/namespace/online.ts";
import { bracket } from "@/bracket.tsx";

/** The object at `url`, whole; `name` is what a warning calls it. */
export async function get(url: string, name: string, signal: AbortSignal): Promise<U8Array> {
  let res: Response;
  let body: U8Array;
  try {
    res = await bracket.origin.get(url, signal);
    body = new Uint8Array(await res.arrayBuffer());
  } catch (err) {
    // TypeError covers offline, DNS, CORS and refused alike, so only the
    // browser's own word says offline; the rest is the server's.
    if (!signal.aborted) console.warn(`[read] ${name}: ${err instanceof Error ? `${err.name}: ${err.message}` : String(err)}`);
    throw new FileError(online() ? Errno.EIO : Errno.ENETUNREACH);
  }
  if (res.status === 204) throw new FileError(Errno.ENOENT);
  if (!res.ok) {
    console.warn(`[read] ${name}: HTTP ${res.status}`);
    // A key that names nothing is the object's failure; any other, the server's.
    throw new FileError(res.status === 404 ? Errno.ENXIO : Errno.EACCES, res.status);
  }
  return body;
}
