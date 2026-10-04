// @pxl/web/kernel/io/fetch.ts

/**
 * The bus under the kernel's reads: the kernel names an object, this side
 * makes it a URL. 404 is ENOENT, not a failure. A body still packed means
 * the browser cannot decode the stored encoding; the Worker cannot refuse it
 * there, since Cloudflare rewrites `accept-encoding` before the Worker sees it.
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import type { f64, u32, u64 } from "@xpute/core/abi/word.ts";
import { Command as XputeCommand, encode_fetch_done, FetchOutcome } from "@xpute/runtime/abi/cmd.spec.ts";
import { FetchFail } from "@xpute/runtime/abi/fetch.spec.ts";
import { PUBLISHED_TILES } from "@/config/deploy.ts";
import { originGet } from "@platform/origin.ts";
import { tileUrl } from "@/config/route.ts";
import { BAKE_KINDS } from "@/config/published.spec.ts";
import { CORE_DOORBELL } from "@/kernel/mem.ts";
import { BODIES } from "@/kernel/module.ts";
import { request_frame } from "@/kernel/sched.ts";

/** Every read the scheduler started and the host has not settled, by the key
 * it handed out, with what stops it. One map for both kinds: a stop names a
 * key and nothing else, so which kind of read it was is not this side's
 * question (load.ts). */
const running = new Map<bigint, () => void>();

/** A read begun under `key`: the signal its transport is stopped by, and
 * `own`, which says whether its end is still this read's to report — and
 * from then on it is not. A read ends once, by its answer or by its stop,
 * whichever comes first: a stop is reported at once, through `stopped`, and
 * never waits on the transport to let the read go. A transport may never let
 * it go — a device's fetch left a body stopped after its response unsettled
 * for good — and the credit the read spends would never come back. */
export function begin_read(key: u64, stopped: () => void): { signal: AbortSignal; own: () => boolean } {
  const controller = new AbortController();
  running.set(key, () => {
    running.delete(key);
    controller.abort();
    stopped();
  });
  return { signal: controller.signal, own: () => running.delete(key) };
}

function packed(body: U8Array): boolean {
  return (body[0] === 0x28 && body[1] === 0xb5) || (body[0] === 0x1f && body[1] === 0x8b);
}

export async function read_object(bake: u32, name: string, signal: AbortSignal): Promise<U8Array> {
  let res: Response;
  let body: U8Array;
  try {
    const t0 = performance.now();
    const res = await originGet(tileUrl(BAKE_KINDS[bake], name, PUBLISHED_TILES), signal);
    const body = new Uint8Array(await res.arrayBuffer());
    net_ms = performance.now() - t0;
    if (!own()) return;
    if (res.status === 404) {
      outcome = FetchOutcome.Absent;
    } else if (!res.ok) {
      outcome = FetchOutcome.Error;
      fail = FetchFail.Refused;
      detail = res.status;
    } else if (body.byteLength > 0 && packed(body)) {
      // Still in the encoding it was stored in: something between the bucket
      // and here dropped the header that says so, since the edge decodes a
      // body for a browser that cannot. The object's failure, not the
      // browser's — and handing it on would spend a heap block and fail in the
      // kernel's decoder, far from the one thing that is actually wrong.
      console.warn(`[fetch] ${key}: the body arrived still packed, with no encoding to undo it`);
      outcome = FetchOutcome.Error;
      fail = FetchFail.Decode;
    } else if (body.byteLength > 0) {
      held = BODIES.hold(body);
      bytes = body.byteLength;
    }
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
