// @pxl/web/kernel/io/fetch.ts

/**
 * The bus under the kernel's reads (crates/pxl/web/kernel/src/io/fetch.rs).
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
 * Four outcomes go back, and only four. **404 is not one of them**: it is how
 * HTTP spells "there is no such object", so it is translated here into the
 * outcome that says so, and the kernel decides what an absent object means
 * for its category without ever learning the number. The body is held where
 * the kernel reads it in its own turn, and a stop is the scheduler's own
 * doing — it carries no reason, where everything else does.
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import type { f64, u32, u64 } from "@xpute/core/abi/word.ts";
import { Command, encode_fetch_update, FetchOutcome } from "@/kernel/abi/cmd.spec.ts";
import { FetchFail } from "@xpute/runtime/abi/fetch.spec.ts";
import { declareUnsupported, isUnsupported } from "@/app/store/unsupported-store.ts";
import { PUBLISHED_TILES } from "@/config/deploy.ts";
import { ORIGIN } from "@platform/origin.ts";
import { tile_url } from "@/config/route.ts";
import { BAKE_KINDS } from "@/config/published.spec.ts";
import { CORE_BUILD } from "@/kernel/ipc.ts";
import { hold_body } from "@/kernel/module.ts";
import { request_frame } from "@/kernel/sched.ts";

/** Every read the scheduler started and the host has not settled, by the key
 * it handed out. One map for both kinds: a stop names a key and nothing else,
 * so which kind of read it was is not this side's question (load.ts). */
export const running = new Map<bigint, AbortController>();

/** Whether a body is still in the encoding it was stored in — which, on this
 * side of the wire, means the transport did not decode it. The same judgment
 * the bake makes on the other side (pxl-geo-bake util.rs's `encoding_of`). */
function packed(body: U8Array): boolean {
  return (body[0] === 0x28 && body[1] === 0xb5) || (body[0] === 0x1f && body[1] === 0x8b);
}

/** Tells the kernel what became of the read it started under `key`. */
function settle(key: u64, outcome: FetchOutcome, body: u32, body_bytes: u32, fail: FetchFail, detail: u32, net_ms: f64): void {
  CORE_BUILD.send(0, Command.FETCH_UPDATE, encode_fetch_update({ key, outcome, body, body_bytes, fail, detail, net_ms }));
  request_frame();
}

/** Reads one object the kernel named: its bytes written into the room the
 * kernel gives them, and what became of the read reported back. */
export async function run_fetch(key: u64, bake: number, name: string): Promise<void> {
  // Settled without being made, once a body has shown that nothing here can be
  // read. The kernel keeps its demand and keeps asking, which is right — the
  // demand is real and nothing about it failed — but the answer would be the
  // same bytes again, and the reader is looking at the notice.
  if (isUnsupported()) return settle(key, FetchOutcome.Error, 0, 0, FetchFail.Shell, 0, 0);
  const controller = new AbortController();
  running.set(key, controller);
  const signal = controller.signal;
  let outcome: FetchOutcome = FetchOutcome.Read;
  let held = 0;
  let bytes = 0;
  let fail: FetchFail = FetchFail.None;
  let detail = 0;
  let net_ms = 0;
  try {
    const t0 = performance.now();
    const res = await fetch(`${ORIGIN}${tile_url(BAKE_KINDS[bake], name, PUBLISHED_TILES)}`, { signal });
    const body = new Uint8Array(await res.arrayBuffer());
    net_ms = performance.now() - t0;
    if (res.status === 404) {
      outcome = FetchOutcome.Absent;
    } else if (!res.ok) {
      outcome = FetchOutcome.Error;
      fail = FetchFail.Refused;
      detail = res.status;
    } else if (body.byteLength > 0 && packed(body)) {
      // Handing these on would spend a heap block and fail in the kernel's
      // decoder, far from the one thing that is actually wrong.
      declareUnsupported("encoding");
      outcome = FetchOutcome.Error;
      fail = FetchFail.Shell;
    } else if (body.byteLength > 0) {
      held = hold_body(body);
      bytes = body.byteLength;
    }
  } catch (err) {
    outcome = signal.aborted ? FetchOutcome.Aborted : FetchOutcome.Error;
    // A stop is the scheduler's own doing and carries no reason. Everything
    // else is the transport's: a browser raises TypeError for offline, DNS,
    // CORS and a refused connection alike, and its words for them are this
    // side's to log — what the kernel needs is which of these happened.
    if (!signal.aborted) {
      fail = err instanceof Error && err.name === "TypeError" ? FetchFail.Network : FetchFail.Shell;
      console.warn(`[fetch] ${key}: ${err instanceof Error ? `${err.name}: ${err.message}` : String(err)}`);
    }
  } finally {
    running.delete(key);
    settle(key, outcome, held, bytes, fail, detail, net_ms);
  }
}

/** Stops the read the kernel started under `key`, whichever kind it is, if it
 * is still running. */
export function abort_fetch(key: u64): void {
  running.get(key)?.abort();
}
