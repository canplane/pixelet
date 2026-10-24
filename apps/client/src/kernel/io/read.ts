// @pxl/client/kernel/io/read.ts

/**
 * The bus under the kernel's reads: the kernel names a bake's object or a
 * shipped asset by number, and this side, pixelet's end of the kernel, makes it
 * a URL (io/object.ts). 204 is a page its version leaves out, ENOENT; 404
 * names nothing, a failure like any other.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/hypervisor/ipc/sys.ts";
import { INGESTED, PACK } from "@/catalog/asset/ingested.spec.ts";
import { BASE_LAYERS } from "@/config/published.spec.ts";
import { assetUrl, tileUrl } from "@/config/route.ts";
import { originGet } from "@/bracket.tsx";
import { online } from "@/kernel/online.ts";
import { tile_object } from "@/kernel/io/object.ts";
import { source_declared } from "@/kernel/io/source.ts";

async function get(url: string, name: string, signal: AbortSignal): Promise<U8Array> {
  let res: Response;
  let body: U8Array;
  try {
    res = await originGet(url, signal);
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

/**
 * A body still packed means the transport did not decode the stored
 * encoding: a browser that cannot, which the Worker cannot refuse, since
 * Cloudflare rewrites `accept-encoding` before the Worker sees it; or a
 * device whose own decode (apps/native/src/origin.ts) was skipped.
 */
function packed(body: U8Array): boolean {
  return (body[0] === 0x28 && body[1] === 0xb5) || (body[0] === 0x1f && body[1] === 0x8b);
}

/** A bake's object by its number; none before the source is declared, as the kernel asks for none. */
export async function read_object(bake: u32, number: bigint, signal: AbortSignal): Promise<U8Array> {
  const declared = source_declared();
  const name = declared === null ? null : tile_object(declared, BASE_LAYERS[bake], number);
  if (name === null) throw new FileError(Errno.ENOENT);
  const body = await get(tileUrl(BASE_LAYERS[bake], name), name, signal);
  if (body.byteLength > 0 && packed(body)) {
    // Fail here: handing it on would fail in the kernel's decoder, far from the cause.
    console.warn(`[read] ${name}: the body arrived still packed, with no encoding to undo it`);
    throw new FileError(Errno.EBADMSG);
  }
  return body;
}

const KEYS: readonly string[] = Object.values(INGESTED).map((a) => a.key);

/** A load number: an asset's index in the ingest's table, or one past it for the texture pack. */
export function read_asset(load: bigint, signal: AbortSignal): Promise<U8Array> {
  const key = load === BigInt(KEYS.length) ? PACK.key : KEYS[Number(load)];
  if (key === undefined) return Promise.reject(new FileError(Errno.ENOENT));
  return get(assetUrl(key), key, signal);
}
