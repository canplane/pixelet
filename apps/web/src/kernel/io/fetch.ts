// @pxl/web/kernel/io/fetch.ts

/**
 * The bus under the kernel's reads: the kernel names an object, this side
 * makes it a URL. 404 is ENOENT, not a failure. A body still packed means
 * the browser cannot decode the stored encoding; the Worker cannot refuse it
 * there, since Cloudflare rewrites `accept-encoding` before the Worker sees it.
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import type { u32 } from "@xpute/core/abi/word.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { FileError } from "@xpute/runtime/ipc/sys.ts";
import { PUBLISHED_TILES } from "@/config/deploy.ts";
import { originGet } from "@platform/origin.ts";
import { tileUrl } from "@/config/route.ts";
import { BAKE_KINDS } from "@/config/published.spec.ts";

function packed(body: U8Array): boolean {
  return (body[0] === 0x28 && body[1] === 0xb5) || (body[0] === 0x1f && body[1] === 0x8b);
}

export async function read_object(bake: u32, name: string, signal: AbortSignal): Promise<U8Array> {
  let res: Response;
  let body: U8Array;
  try {
    res = await originGet(tileUrl(BAKE_KINDS[bake], name, PUBLISHED_TILES), signal);
    body = new Uint8Array(await res.arrayBuffer());
  } catch (err) {
    // TypeError covers offline, DNS, CORS and refused alike; the words are only logged.
    if (!signal.aborted) console.warn(`[fetch] ${name}: ${err instanceof Error ? `${err.name}: ${err.message}` : String(err)}`);
    throw new FileError(err instanceof Error && err.name === "TypeError" ? Errno.ENETUNREACH : Errno.EIO);
  }
  if (res.status === 404) throw new FileError(Errno.ENOENT);
  if (!res.ok) throw new FileError(Errno.EACCES, res.status);
  if (body.byteLength > 0 && packed(body)) {
    // Fail here: handing it on would fail in the kernel's decoder, far from the cause.
    console.warn(`[fetch] ${name}: the body arrived still packed, with no encoding to undo it`);
    throw new FileError(Errno.EBADMSG);
  }
  return body;
}
