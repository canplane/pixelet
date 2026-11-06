// @pxl/client/service/runtime.ts

/**
 * A page's runtime, in the kernel's worker (runtime/worker.ts), so a turn holds
 * none of the shell's events (service/runtime.native.ts is a device's): the
 * kernel's commands cross xpute's remote as frames, the snapshot comes back as
 * bytes, and what the worker's devices say is kept here for the shell.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { PacketWriter, type PacketRoom } from "@xpute/kit/wire/xtp/mod.ts";
import { connect } from "@xpute/runtime";
import { Command, shell_imports } from "@/abi/cmd.spec.ts";
import { MACHINE } from "@/abi/machine.spec.ts";
import { mirror_source } from "@/sys/fs/source.ts";
import { onabort } from "@/sys/abort.ts";
import { set_online } from "@/origin/online.ts";
import { kernel, set_kernel } from "@/runtime/kernel.ts";
import type { ToKernel, ToShell } from "@/runtime/page.ts";
import { SENSOR_LISTENER, sensor_start, sensor_stop } from "@/port/sensor.ts";
import { GPU_STATE, set_gpu } from "@/port/gpu.ts";
import * as snapshot from "@/port/snapshot.ts";
import * as sensor from "@/service/sensor.ts";
import * as visibility from "@/service/visibility.ts";
import * as storage from "@/service/storage.ts";
import type { OnOpen } from "@/service/types.ts";

/** The keys the kernel's files are kept under (sys/fs/storage.ts). */
const KEY_PREFIX = "pxl:";

/** The runtime started in its worker and the kernel's commands bound to it;
 * settled once a GPU has answered and the world's source is declared, true
 * where it was. */
export function open(on: OnOpen): Promise<boolean> {
  const worker = new Worker(new URL("../runtime/worker.ts", import.meta.url), {
    type: "module",
  });
  const ask = (m: ToKernel, moved: Transferable[] = []) =>
    worker.postMessage(m, moved);
  const { port1, port2 } = new MessageChannel();
  const remote = connect(port1, MACHINE.slot_bytes);
  remote.onterminated = on.terminated;
  const on_import = shell_imports({
    shell_snapshot: () => {},
    shell_view: snapshot.view_posted,
    sensor_start,
    sensor_stop,
    text_layout: () => Errno.ENOSYS,
    text_raster: () => Errno.ENOSYS,
  });
  remote.listen((tag, cmd, f, named) => {
    if (cmd !== Command.SHELL_SNAPSHOT)
      return void on_import(tag, cmd, f, null);
    if (named) snapshot.set_snapshot(named);
    on.snapshot();
  });
  // A canvas transfers once: an effect mounted again lends the same one.
  const offscreens = new WeakMap<HTMLCanvasElement, OffscreenCanvas>();
  const room = scratch(MACHINE.slot_bytes);
  const reads = new Map<u32, (value: unknown) => void>();
  let next_read: u32 = 0;
  set_kernel(remote, {
    resume: () => ask({ kind: "resume" }),
    pause: () => ask({ kind: "pause" }),
    wake: () => ask({ kind: "wake" }),
    input: () => ask({ kind: "input" }),
    visible: (visible) => ask({ kind: "visible", visible }),
    fits: (encode) => {
      const pkt = encode(room);
      return pkt === null || pkt.finish() >= 0;
    },
    cmd: (cmd, ...args) => ask({ kind: "cmd", cmd, args }),
    bind: (handle: u32, canvas: HTMLCanvasElement) => {
      let offscreen = offscreens.get(canvas);
      if (offscreen) return ask({ kind: "bind", handle, canvas: offscreen });
      offscreen = canvas.transferControlToOffscreen();
      offscreens.set(canvas, offscreen);
      ask({ kind: "bind", handle, canvas: offscreen }, [offscreen]);
    },
    unbind: (handle) => ask({ kind: "unbind", handle }),
    perf: (what, args) => {
      next_read = (next_read + 1) >>> 0 || 1;
      const id = next_read;
      return new Promise((resolve) => {
        reads.set(id, resolve);
        ask({ kind: "perf", id, what, args });
      });
    },
  });
  const keys: [string, string][] = [];
  for (let i = 0; i < localStorage.length; i++) {
    const key = localStorage.key(i);
    const value = key?.startsWith(KEY_PREFIX) ? storage.get(key) : null;
    if (key && value !== null) keys.push([key, value]);
  }
  ask({ kind: "start", frames: port2, keys }, [port2]);
  return new Promise((resolve, reject) => {
    worker.onerror = (e) => reject(new Error(e.message));
    worker.onmessage = ({ data }: MessageEvent<ToShell>) => {
      switch (data.kind) {
        case "started":
          on.started();
          sensor.listen(SENSOR_LISTENER);
          visibility.watch((visible) => ask({ kind: "visible", visible }));
          {
            const { geo, orient } = sensor;
            kernel.sensor_platform(
              geo.has,
              orient.has,
              orient.gated,
              orient.silence,
            );
            kernel.setting_development(__IS_DEV__);
          }
          return;
        case "opened":
          return resolve(data.declared);
        case "failed":
          return reject(new Error(data.why));
        case "gpu":
          return set_gpu(GPU_STATE[data.state]);
        case "online":
          return set_online(data.online);
        case "source":
          return mirror_source(data.state, data.declaration);
        case "abort":
          return onabort(data.report);
        case "keep":
          return storage.set(data.key, data.value);
        case "perf": {
          const done = reads.get(data.id);
          reads.delete(data.id);
          return done?.(data.value);
        }
      }
    };
  });
}

/** Where a command's packet is written to see whether it fits a slot. */
function scratch(bytes: u32): PacketRoom {
  const buf = new Uint8Array(bytes);
  const view = new DataView(buf.buffer);
  const w = new PacketWriter();
  return { writer: (cnt) => w.begin(buf, view, 0, bytes, cnt) };
}
