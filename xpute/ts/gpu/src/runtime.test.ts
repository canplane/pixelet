// @xpute/gpu/runtime.test.ts

/**
 * Holds the web runtime to golden/gpu/program.tsv where it runs: in a
 * browser, on its WebGPU. Each program's recorded turns, run as they were
 * written, must read back the bytes the record holds, as the native runtime's
 * do (rust/gpu/src/runtime.test.rs). Turn K's ticket is K + 1, and the next
 * turn waits until the runtime has stored it at the fence. The test also holds
 * that a surface's handle keeps its slot only while it is the one bound there.
 *
 * ts/gpu/script/browser.ts serves it to Chrome, and the page posts its outcome
 * back: "ok", or what failed.
 */

import { Golden } from "@xpute/kit/golden.ts";

import { GpuRuntime } from "./runtime.ts";

function check(ok: boolean, msg: string): asserts ok {
  if (!ok) throw new Error(msg);
}

/** Checks that `seen` holds the values of `want` in the same order, and
 * otherwise names the first place they part. */
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
  const g = await Golden.load("golden/gpu/program.tsv");
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) throw new Error("no WebGPU adapter");
  const device = await adapter.requestDevice({
    requiredFeatures: ["timestamp-query"],
  });
  const [region, inputAt, done, readback] = [
    "region",
    "input.at",
    "done.at",
    "readback.at",
  ].map((k) => g.u32(k));
  const names: string[] = [];
  g.each("program", (k) => names.push(g.s(k)));
  for (const p of names) {
    const mem = new ArrayBuffer(region);
    const bytes = new Uint8Array(mem);
    const words = new Uint32Array(mem);
    if (g.has(`${p}.input`)) bytes.set(g.bytes(`${p}.input`), inputAt);
    const runtime = new GpuRuntime(
      device,
      () => mem,
      () => {},
    );
    const turns: string[] = [];
    g.each(`${p}.turn`, (k) => turns.push(k));
    for (const [k, turn] of turns.entries()) {
      bytes.set(g.bytes(turn), 0);
      runtime.run(0);
      while (Atomics.load(words, done / 4) !== k + 1) {
        await new Promise((r) => setTimeout(r, 1));
      }
    }
    const want = g.bytes(`${p}.readback`);
    same(bytes.slice(readback, readback + want.length), want, `${p}.readback`);
  }
  device.destroy();
}

/** An old handle's late unbind lets go no newer surface in its slot, and a
 * context that loses its slot to a newer handle is let go then. Binding
 * reaches neither the device nor a canvas, so both are stand-ins that record
 * what is let go. */
function guard(): void {
  const released: string[] = [];
  const context = (name: string) =>
    ({ unconfigure: () => released.push(name) }) as unknown as GPUCanvasContext;
  const device = { lost: new Promise<never>(() => {}) } as unknown as GPUDevice;
  const runtime = new GpuRuntime(
    device,
    () => new ArrayBuffer(0),
    () => {},
  );
  // These are slot 3, generations 1 and 2 (`slot | generation << 16`).
  const [old, newer] = [3 | (1 << 16), 3 | (2 << 16)];
  runtime.bind(old, context("old"));
  runtime.bind(newer, context("newer"));
  same(released, ["old"], "the slot's old context let go as the newer took it");
  check(
    !runtime.bind(old, context("late")),
    "an old handle's late bind takes nothing back",
  );
  same(released, ["old"]);
  check(!runtime.unbind(old), "an old handle unbinds nothing");
  same(released, ["old"]);
  check(runtime.unbind(newer), "the newer handle unbinds its own");
  same(released, ["old", "newer"]);
  check(!runtime.unbind(newer), "and only once");
  check(
    !runtime.bind(old, context("late")),
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
