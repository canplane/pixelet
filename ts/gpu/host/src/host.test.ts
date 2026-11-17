// @pxl/gpu/host/src/host.test.ts

/**
 * Holds the web GPU host to spec/golden/gpu/program.tsv where it
 * runs: in a browser, on its WebGPU. Each program's recorded turns, each the
 * records `gpu.run` is lent, run as the web `gpu.run` runs them (web.ts),
 * must read back the bytes the record holds, as the native host's do
 * (pxl-gpu-host lib.test.rs). Turn K's ticket is K + 1, and the next
 * turn waits until the host has stored it at the fence.
 *
 * script/browser.ts serves it to Chrome, handing the golden reader where the
 * page serves the records, and the page posts its outcome back: "ok", or what
 * failed.
 */

import { Golden } from "@xpute/golden";
import { rebase } from "../../link/src/addr.ts";

import { GpuHost } from "./host.ts";

/** Checks that `seen` holds the values of `want` in the same order, and
 * otherwise names the first place they part. */
function same(
  seen: ArrayLike<unknown>,
  want: ArrayLike<unknown>,
  message: string,
): void {
  const at =
    seen.length === want.length
      ? Array.from(seen).findIndex((val, i) => val !== want[i])
      : Math.min(seen.length, want.length);
  if (at !== -1)
    throw new Error(
      `${message}: they part at ${at} of ${seen.length} and ${want.length}`,
    );
}

async function replay(): Promise<void> {
  const g = await Golden.load("spec/golden/gpu/program.tsv");
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) throw new Error("no WebGPU adapter");
  const device = await adapter.requestDevice({
    requiredFeatures: ["timestamp-query"],
  });
  const [region_sz, recs_at, input_at, done_at, readback_at] = [
    "region",
    "recs.at",
    "input.at",
    "done.at",
    "readback.at",
  ].map((k) => g.u32(k));
  const names: string[] = [];
  g.each("program", (k) => names.push(g.s(k)));
  for (const p of names) {
    const mem = new ArrayBuffer(region_sz);
    const bytes = new Uint8Array(mem);
    const words = new Uint32Array(mem);
    if (g.has(`${p}.input`)) bytes.set(g.bytes(`${p}.input`), input_at);
    const host = new GpuHost(
      device,
      () => mem,
      () => {},
      () => {},
      { frame_latency: 2, upload_chunk_sz: 1 << 16 },
    );
    const turns: string[] = [];
    g.each(`${p}.turn`, (k) => turns.push(k));
    for (const [k, turn] of turns.entries()) {
      const recs = g.bytes(turn);
      bytes.set(recs, recs_at);
      const turn_words = new Uint32Array(mem, recs_at, recs.length / 4);
      // An address is a pointer natively, and so differs every
      // run: the record holds each relative to its block, and the addresses
      // the stream spec's ops name move to this block, whose base on the web
      // is the memory's start.
      rebase(turn_words, 0n, 0n);
      host.run(turn_words);
      while (Atomics.load(words, done_at / 4) !== k + 1) {
        await new Promise((r) => setTimeout(r, 1));
      }
    }
    const want = g.bytes(`${p}.readback`);
    same(
      bytes.slice(readback_at, readback_at + want.length),
      want,
      `${p}.readback`,
    );
  }
  device.destroy();
}

const outcome = await replay().then(
  () => "ok",
  (e: unknown) => (e instanceof Error ? `${e.message}\n${e.stack}` : String(e)),
);
await fetch("/outcome", { method: "POST", body: outcome });
