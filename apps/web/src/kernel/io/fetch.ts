// @pxl/web/kernel/io/fetch.ts

/**
 * HTTP for the kernel (crates/pxl/web/kernel/src/io/fetch.rs): the kernel
 * names a URL, this runs it, and the status and the bytes are reported back
 * for the kernel to decide what they mean.
 *
 * A fetch is HTTP and nothing more. The body is held where the kernel reads
 * it in its own turn, and an abort is the scheduler's own doing — it carries
 * no reason, where everything else does.
 */

import type { u64 } from "@xpute/core/abi/word.ts";
import { Command } from "@/kernel/abi/cmd.spec.ts";
import { FetchFail } from "@xpute/runtime/abi/fetch.spec.ts";
import { CORE_BUILD, packet } from "@/kernel/ipc.ts";
import { hold_body } from "@/kernel/module.ts";
import { request_frame } from "@/kernel/sched.ts";

const running = new Map<bigint, AbortController>();

/** Runs one fetch the kernel started: the response's bytes written into the
 * room the kernel gives them, and the status reported. */
export async function run_fetch(key: u64, url: string): Promise<void> {
  const controller = new AbortController();
  running.set(key, controller);
  const signal = controller.signal;
  let outcome = 0;
  let status = 0;
  let held = 0;
  let bytes = 0;
  let fail: FetchFail = FetchFail.None;
  let net_ms = 0;
  try {
    const t0 = performance.now();
    const res = await fetch(url, { signal });
    const body = new Uint8Array(await res.arrayBuffer());
    net_ms = performance.now() - t0;
    status = res.status;
    if (body.byteLength > 0) {
      held = hold_body(body);
      bytes = body.byteLength;
    }
  } catch (err) {
    outcome = signal.aborted ? 1 : 2;
    // An abort is the scheduler's own doing and carries no reason. Everything
    // else is the transport's: a browser raises TypeError for offline, DNS,
    // CORS and a refused connection alike, and its words for them are this
    // side's to log — what the kernel needs is which of these happened.
    if (!signal.aborted) {
      fail = err instanceof Error && err.name === "TypeError" ? FetchFail.Network : FetchFail.Shell;
      console.warn(`[fetch] ${key}: ${err instanceof Error ? `${err.name}: ${err.message}` : String(err)}`);
    }
  } finally {
    running.delete(key);
    CORE_BUILD.send(0, Command.FETCH_UPDATE, packet((b) => b.u64(key).u32(outcome).u32(status).u32(held).u32(bytes).u32(fail).f64(net_ms)));
    request_frame();
  }
}

/** Stops the fetch the kernel started under `key`, if it is still running. */
export function abort_fetch(key: u64): void {
  running.get(key)?.abort();
}
