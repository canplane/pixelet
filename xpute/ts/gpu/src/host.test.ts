// @xpute/gpu/host.test.ts

/**
 * Holds the web host to golden/gpu/programs.tsv where it runs, in a browser
 * on its WebGPU: each program's recorded turns, run as they were written,
 * read back the bytes the record holds, as the native host's do
 * (rust/gpu/src/host.test.rs). Turn K's ticket, K + 1, stands at the done
 * word as the guest left it, and the next turn waits for its DONE.
 *
 * script/web-gpu.ts serves it to Chrome, and the page posts its outcome
 * back: "ok", or what failed.
 */

import { assertEquals } from "@std/assert";

import { Golden } from "@xpute/kit/golden.ts";

import { DONE } from "./api.spec.ts";
import { GpuHost } from "./host.ts";

async function replay(): Promise<void> {
  const g = await Golden.load("golden/gpu/programs.tsv");
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) throw new Error("no WebGPU adapter");
  const device = await adapter.requestDevice({ requiredFeatures: ["timestamp-query"] });
  const [region, head, inputAt, done, readback] = ["region", "stream.head", "input.at", "done.at", "readback.at"].map((k) => g.u32(k));
  const names: string[] = [];
  g.each("program", (k) => names.push(g.s(k)));
  for (const p of names) {
    const memory = new ArrayBuffer(region);
    const bytes = new Uint8Array(memory);
    const words = new Uint32Array(memory);
    if (g.has(`${p}.input`)) bytes.set(g.bytes(`${p}.input`), inputAt);
    const host = new GpuHost(device, () => memory, () => {});
    const turns: string[] = [];
    g.each(`${p}.turn`, (k) => turns.push(k));
    for (const [k, turn] of turns.entries()) {
      bytes.set(g.bytes(turn), 0);
      words[done / 4] = k + 1;
      host.run(0, head);
      while ((words[done / 4] & DONE) === 0) await new Promise((r) => setTimeout(r, 1));
    }
    const want = g.bytes(`${p}.readback`);
    assertEquals(bytes.slice(readback, readback + want.length), want, `${p}.readback`);
  }
  device.destroy();
}

const outcome = await replay().then(() => "ok", (e: unknown) => (e instanceof Error ? `${e.message}\n${e.stack}` : String(e)));
await fetch("/outcome", { method: "POST", body: outcome });
