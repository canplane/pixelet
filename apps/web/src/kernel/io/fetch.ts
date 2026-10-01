// @pxl/web/kernel/io/fetch.ts

/**
 * The bus under the kernel's reads (crates/pxl/web/kernel/src/io/fetch.rs):
 * a file below a bake's root, opened whole.
 *
 * The kernel names an object — which bake it belongs to, and its name inside
 * that bake — and never an address. Turning that into a URL, running HTTP,
 * and reading what the protocol says are all this side's: a kernel names what
 * it wants and a driver knows the bus.
 *
 * The other thing only this side can see is a body the transport did not
 * decode. A page is served in the encoding it was stored in, and a browser
 * that does not know that encoding is handed the bytes undecoded rather than
 * told anything — so what says the browser cannot read this deployment is the
 * first two bytes of a body that should already be a packet. The Worker cannot
 * say it: Cloudflare rewrites `accept-encoding` on the request a Worker is
 * handed and negotiates with the real client itself, so a refusal written
 * there refuses everyone (2026-09-24).
 *
 * **404 is not a failure**: it is how HTTP spells "there is no such object",
 * so it is ENOENT here, and the kernel decides what an absent object means for
 * its category without ever learning the number. A stop is the kernel closing
 * the file, and nothing is said of it.
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import type { u32 } from "@xpute/core/abi/word.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { FileError } from "@xpute/runtime/ipc/sys.ts";
import { PUBLISHED_TILES } from "@/config/deploy.ts";
import { originGet } from "@platform/origin.ts";
import { tileUrl } from "@/config/route.ts";
import { BAKE_KINDS } from "@/config/published.spec.ts";

/** Whether a body is still in the encoding it was stored in — which, on this
 * side of the wire, means the transport did not decode it. The same judgment
 * the bake makes on the other side (pxl-geo-bake util.rs's `encoding_of`). */
function packed(body: U8Array): boolean {
  return (body[0] === 0x28 && body[1] === 0xb5) || (body[0] === 0x1f && body[1] === 0x8b);
}

/** The object `name` of bake `bake`, whole, or a FileError. */
export async function read_object(bake: u32, name: string, signal: AbortSignal): Promise<U8Array> {
  let res: Response;
  let body: U8Array;
  try {
    res = await originGet(tileUrl(BAKE_KINDS[bake], name, PUBLISHED_TILES), signal);
    body = new Uint8Array(await res.arrayBuffer());
  } catch (err) {
    // A browser raises TypeError for offline, DNS, CORS and a refused
    // connection alike, and its words for them are this side's to log —
    // what the kernel needs is which of these happened.
    if (!signal.aborted) console.warn(`[fetch] ${name}: ${err instanceof Error ? `${err.name}: ${err.message}` : String(err)}`);
    throw new FileError(err instanceof Error && err.name === "TypeError" ? Errno.ENETUNREACH : Errno.EIO);
  }
  if (res.status === 404) throw new FileError(Errno.ENOENT);
  if (!res.ok) throw new FileError(Errno.EACCES, res.status);
  if (body.byteLength > 0 && packed(body)) {
    // Still in the encoding it was stored in: something between the bucket
    // and here dropped the header that says so, since the edge decodes a
    // body for a browser that cannot. The object's failure, not the
    // browser's — and handing it on would spend a heap block and fail in the
    // kernel's decoder, far from the one thing that is actually wrong.
    console.warn(`[fetch] ${name}: the body arrived still packed, with no encoding to undo it`);
    throw new FileError(Errno.EBADMSG);
  }
  return body;
}
