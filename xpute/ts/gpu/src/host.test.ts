// @xpute/gpu/host.test.ts

/**
 * Holds the web host to golden/gpu/programs.tsv where it runs, in a browser
 * on its WebGPU: each program's recorded turns, run as they were written,
 * read back the bytes the record holds, as the native host's do
 * (rust/gpu/src/host.test.rs). Turn K's ticket, K + 1, stands at the done
 * word as the guest left it, and the next turn waits for its DONE. And a
 * surface's handle holds its slot only while it is the one bound there.
 *
 * ts/gpu/script/browser.ts serves it to Chrome, and the page posts its outcome
 * back: "ok", or what failed.
 */

import { Golden } from "@xpute/kit/golden.ts";

import { DONE } from "./api.spec.ts";
import { GpuHost } from "./host.ts";

// Its own checks: the page it runs in has no node:assert.
function check(ok: boolean, msg: string): asserts ok {
  if (!ok) throw new Error(msg);
}

/** The same values in the same order; where not, the first place they part. */
function same(
  seen: ArrayLike<unknown>,
  want: ArrayLike<unknown>,
  msg = "not the same values",
): void {
  const at =
    seen.length === want.length
      ? Array.from(seen).findIndex((val, i) => val !== want[i])
      : Math.min(seen.length, want.length);
  check(
    at === -1,
    `${msg}: they part at ${at} of ${seen.length} and ${want.length}`,
  );
}

async function replay(): Promise<void> {
  const g = await Golden.load("golden/gpu/programs.tsv");
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) throw new Error("no WebGPU adapter");
  const device = await adapter.requestDevice({
    requiredFeatures: ["timestamp-query"],
  });
  const [region, head, inputAt, done, readback] = [
    "region",
    "stream.head",
    "input.at",
    "done.at",
    "readback.at",
  ].map((k) => g.u32(k));
  const names: string[] = [];
  g.each("program", (k) => names.push(g.s(k)));
  for (const p of names) {
    const memory = new ArrayBuffer(region);
    const bytes = new Uint8Array(memory);
    const words = new Uint32Array(memory);
    if (g.has(`${p}.input`)) bytes.set(g.bytes(`${p}.input`), inputAt);
    const host = new GpuHost(
      device,
      () => memory,
      () => {},
    );
    const turns: string[] = [];
    g.each(`${p}.turn`, (k) => turns.push(k));
    for (const [k, turn] of turns.entries()) {
      bytes.set(g.bytes(turn), 0);
      words[done / 4] = k + 1;
      host.run(0, head);
      while ((words[done / 4] & DONE) === 0) {
        await new Promise((r) => setTimeout(r, 1));
      }
    }
    const want = g.bytes(`${p}.readback`);
    same(bytes.slice(readback, readback + want.length), want, `${p}.readback`);
  }
  device.destroy();
}

/** An old handle's late unbind lets go no newer surface in its slot, and a
 * context taken from its slot is let go then. Binding reaches neither the
 * device nor a canvas, so both are stand-ins that record what is let go. */
function guard(): void {
  const released: string[] = [];
  const context = (name: string) =>
    ({ unconfigure: () => released.push(name) }) as unknown as GPUCanvasContext;
  const device = { lost: new Promise<never>(() => {}) } as unknown as GPUDevice;
  const host = new GpuHost(
    device,
    () => new ArrayBuffer(0),
    () => {},
  );
  // Slot 3, generations 1 and 2 (`slot | generation << 16`).
  const [old, newer] = [3 | (1 << 16), 3 | (2 << 16)];
  host.bind(old, context("old"));
  host.bind(newer, context("newer"));
  same(released, ["old"], "the slot's old context let go as the newer took it");
  check(
    !host.bind(old, context("late")),
    "an old handle's late bind takes nothing back",
  );
  same(released, ["old"]);
  check(!host.unbind(old), "an old handle unbinds nothing");
  same(released, ["old"]);
  check(host.unbind(newer), "the newer handle unbinds its own");
  same(released, ["old", "newer"]);
  check(!host.unbind(newer), "and only once");
  check(
    !host.bind(old, context("late")),
    "the slot remembers the newer after it is let go",
  );
}

const outcome = await replay()
  .then(guard)
  .then(
    () => "ok",
    (e: unknown) =>
      e instanceof Error ? `${e.message}\n${e.stack}` : String(e),
  );
await fetch("/outcome", { method: "POST", body: outcome });
