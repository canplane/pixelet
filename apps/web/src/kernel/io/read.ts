// @pxl/web/kernel/io/read.ts

/**
 * The bus under the kernel's reads: the kernel names a baked object or a
 * shipped asset, this side makes it a URL. 204 is a page its version leaves
 * out, ENOENT; 404 names nothing, a failure like any other.
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import type { u32 } from "@xpute/core/abi/word.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { FileError } from "@xpute/host/ipc/sys.ts";
import { INGESTED, PACK } from "@/catalog/asset/ingested.spec.ts";
import { PUBLISHED_TILES } from "@/config/deploy.ts";
import { BAKE_KINDS } from "@/config/published.spec.ts";
import { assetUrl, tileUrl } from "@/config/route.ts";
import { originGet } from "@platform/origin.ts";

async function get(url: string, name: string, signal: AbortSignal): Promise<U8Array> {
  let res: Response;
  let body: U8Array;
  try {
    res = await originGet(url, signal);
    body = new Uint8Array(await res.arrayBuffer());
  } catch (err) {
    // TypeError covers offline, DNS, CORS and refused alike; the words are only logged.
    if (!signal.aborted) console.warn(`[read] ${name}: ${err instanceof Error ? `${err.name}: ${err.message}` : String(err)}`);
    throw new FileError(err instanceof Error && err.name === "TypeError" ? Errno.ENETUNREACH : Errno.EIO);
  }
  if (res.status === 204) throw new FileError(Errno.ENOENT);
  if (!res.ok) throw new FileError(Errno.EACCES, res.status);
  return body;
}

/**
 * A body still packed means the browser cannot decode the stored encoding;
 * the Worker cannot refuse it there, since Cloudflare rewrites
 * `accept-encoding` before the Worker sees it.
 */
function packed(body: U8Array): boolean {
  return (body[0] === 0x28 && body[1] === 0xb5) || (body[0] === 0x1f && body[1] === 0x8b);
}

export async function read_object(bake: u32, name: string, signal: AbortSignal): Promise<U8Array> {
  const body = await get(tileUrl(BAKE_KINDS[bake], name, PUBLISHED_TILES), name, signal);
  if (body.byteLength > 0 && packed(body)) {
    // Fail here: handing it on would fail in the kernel's decoder, far from the cause.
    console.warn(`[read] ${name}: the body arrived still packed, with no encoding to undo it`);
    throw new FileError(Errno.EBADMSG);
  }
  return body;
}

const KEYS: readonly string[] = Object.values(INGESTED).map((a) => a.key);

/** A load number: an asset's index in the ingest's table, or one past it for the texture pack. */
export function read_asset(load: u32, signal: AbortSignal): Promise<U8Array> {
  const key = load === KEYS.length ? PACK.key : KEYS[load];
  if (key === undefined) return Promise.reject(new FileError(Errno.ENOENT));
  return get(assetUrl(key), key, signal);
}
