// @pxl/client/runtime/worker.ts

/**
 * A page's core and runtime, in a worker of their own so a turn holds none
 * of the shell's events (service/runtime.ts starts it): the kernel's module, the
 * files, the GPU on the canvas the shell lends, the text, the network and the
 * frame pacing, on the worker's animation frames. The shell's frames come over
 * xpute's remote; what the kernel raises for the shell (the snapshot, a sensor
 * started or stopped) goes back over it, the snapshot's bytes copied before
 * the runtime's head passes it. What the worker's devices say for the shell to
 * show goes as runtime/page.ts says.
 */

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import type { U8Array } from "@xpute/kit/abi/array.ts";
import { close, serve } from "@xpute/runtime";
import type { OnImport } from "@xpute/runtime";
import { Command, read_shell_snapshot, shell_imports } from "@/abi/cmd.spec.ts";
import {
  declare_source,
  source_declaration,
  source_state,
  watch_source,
} from "@/sys/fs/source.ts";
import { follow_refused_files } from "@/sys/fs/refusal.ts";
import { set_storage } from "@/sys/fs/storage.ts";
import { set_online } from "@/origin/online.ts";
import { control, runtime, set_kernel, set_runtime } from "@/runtime/kernel.ts";
import { start } from "@/runtime/start.ts";
import type { ToKernel, ToShell } from "@/runtime/page.ts";
import { text_layout, text_raster } from "@/port/text.ts";
import { GPU_STATE, gpu_present, set_gpu } from "@/port/gpu.ts";
import * as snapshot from "@/port/snapshot.ts";
import { answer, type PerfRead } from "@/port/perf.ts";
import { gpu } from "@/service/gpu.ts";
import * as online from "@/service/online.ts";
import { SOURCE_DECLARED } from "@/abi/state.ts";

const tell = (m: ToShell) => postMessage(m);

/** What the kernel posts for the shell. */
const FOR_SHELL = new Set<number>([
  Command.SHELL_SNAPSHOT,
  Command.SHELL_VIEW,
  Command.SENSOR_START,
  Command.SENSOR_STOP,
]);

/** The snapshot's bytes, where its frame says they lie. */
function snapshot_of(e: { at: number; len: number }): U8Array {
  return new Uint8Array(runtime.core.mem, e.at, e.len);
}

async function begin(
  frames: MessagePort,
  entries: [string, string][],
): Promise<void> {
  const keys = new Map(entries);
  set_storage({
    get: (key) => keys.get(key) ?? null,
    set: (key, value) => {
      keys.set(key, value);
      tell({ kind: "keep", key, value });
    },
  });
  let raise: OnImport = () => {};
  const own = shell_imports({
    // The worker keeps the snapshot too: the files it names refused are its own.
    shell_snapshot: (e) => {
      snapshot.set_snapshot(snapshot_of(e));
      follow_refused_files(snapshot.current().refused_files);
    },
    shell_view: () => {},
    sensor_start: () => {},
    sensor_stop: () => {},
    text_layout,
    text_raster,
  });
  const program: OnImport = (tag, cmd, f, pkt) => {
    const answer = own(tag, cmd, f, pkt);
    if (FOR_SHELL.has(cmd)) raise(tag, cmd, f, pkt);
    return answer;
  };
  const started = await start(
    program,
    (report) => tell({ kind: "abort", report }),
    () => close(frames),
  );
  const { pacing } = started;
  const r = started.runtime;
  set_runtime(r);
  set_kernel(r, {
    resume: () => r.resume(),
    pause: () => r.pause(),
    wake: () => r.wake(),
    input: () => pacing.input(performance.now()),
    visible: (visible) => r.visible(visible),
    fits: (encode) => {
      const pkt = encode(r.doorbell);
      return pkt === null || r.doorbell.fits(pkt);
    },
    cmd: (cmd, ...args) => r.doorbell.cmd(cmd, ...args),
    bind: (handle, canvas) => gpu.bind(handle, canvas),
    unbind: (handle) => gpu.unbind(handle),
    perf: (what, args) => Promise.resolve(answer(what as PerfRead, args)),
  });
  raise = serve(r, frames, (cmd, f) =>
    cmd === Command.SHELL_SNAPSHOT && f
      ? snapshot_of(read_shell_snapshot(f)).slice()
      : null,
  );
  onmessage = ({ data }: MessageEvent<ToKernel>) => on_shell(data);
  tell({ kind: "started" });
  gpu.watch((state) => {
    set_gpu(GPU_STATE[state]);
    tell({ kind: "gpu", state });
  });
  online.watch((is) => {
    set_online(is);
    tell({ kind: "online", online: is });
  });
  watch_source(() =>
    tell({
      kind: "source",
      state: source_state(),
      declaration: source_declaration(),
    }),
  );
  const declared = declare_source();
  await gpu_present();
  await declared;
  tell({ kind: "opened", declared: source_state() === SOURCE_DECLARED });
}

/** What the shell asks of the runtime, done here as a device's shell does it. */
function on_shell(m: ToKernel): void {
  switch (m.kind) {
    case "resume":
      return control.resume();
    case "pause":
      return control.pause();
    case "wake":
      return control.wake();
    case "input":
      return control.input();
    case "visible":
      return control.visible(m.visible);
    case "cmd":
      return control.cmd(m.cmd, ...m.args);
    case "bind":
      return gpu.bind(m.handle, m.canvas);
    case "unbind":
      return gpu.unbind(m.handle);
    case "perf":
      return tell({
        kind: "perf",
        id: m.id,
        value: answer(m.what as PerfRead, m.args),
      });
    case "start":
      return;
  }
}

onmessage = ({ data }: MessageEvent<ToKernel>) => {
  if (data.kind !== "start") return;
  begin(data.frames, data.keys).catch((err) =>
    tell({ kind: "failed", why: String(err?.message ?? err ?? Errno.EIO) }),
  );
};
