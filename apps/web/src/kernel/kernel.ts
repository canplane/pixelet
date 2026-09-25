// @pxl/web/kernel/kernel.ts

/**
 * The completion ring's signals, and the boot that starts everything.
 *
 * A turn leaves on the completion ring what the kernel asks of the host — the
 * sensors' watches, the fetches to start and cancel, the loads, the UI's
 * snapshot when it changed — and this is where each is handed to whoever runs
 * it. Nothing is decided here: every branch is one line to the module that
 * does the work.
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import type { BranchCursor } from "@xpute/core/wire/xtp/mod.ts";
import {
  Command,
  encode_gpu_create,
  encode_world_create,
  LoadKind,
  read_fetch_create,
  read_fetch_delete,
  read_load_create,
  read_sensor_create,
  read_sensor_delete,
  read_world_snapshot,
  Sensor,
} from "@/kernel/abi/cmd.spec.ts";
import { HANDHELD } from "@/shared/lib/browser/handheld.ts";
import { geolocation_start, geolocation_stop, orientation_listen, orientation_needs_permission, orientation_remove, orientation_supported } from "@/kernel/control/sensor.ts";
import { abort_fetch, run_fetch } from "@/kernel/io/fetch.ts";
import { load_image, load_model } from "@/kernel/io/load.ts";
import { CORE_BUILD } from "@/kernel/ipc.ts";
import { CORE_KERNEL } from "@/kernel/mem.ts";
import { bytes_at } from "@/kernel/module.ts";
import { declareUnsupported } from "@/app/store/unsupported-store.ts";
import { acquire_device } from "@/kernel/render/gpu.ts";
import { request_frame } from "@/kernel/sched.ts";
import { set_snapshot } from "@/kernel/shell.ts";

function on_signal(_tag: u32, cmd: Command, f: BranchCursor | null): void {
  switch (cmd) {
    case Command.WORLD_SNAPSHOT: {
      // The packet is far past a ring frame, so the kernel names the block
      // it holds it in. It is the kernel's and stays valid until it writes
      // the next one, which is a turn away.
      const { at, len } = read_world_snapshot(f!);
      set_snapshot(bytes_at(CORE_KERNEL.memory, at, len));
      return;
    }
    case Command.WORLD_UPDATE:
      return;
    case Command.SENSOR_CREATE:
      if (read_sensor_create(f!).sensor === Sensor.Geolocation) geolocation_start();
      else orientation_listen();
      return;
    case Command.SENSOR_DELETE:
      if (read_sensor_delete(f!).sensor === Sensor.Geolocation) geolocation_stop();
      else orientation_remove();
      return;
    case Command.FETCH_CREATE: {
      const { key, bake, name } = read_fetch_create(f!);
      void run_fetch(key, bake, name);
      return;
    }
    case Command.FETCH_DELETE:
      abort_fetch(read_fetch_delete(f!).key);
      return;
    case Command.LOAD_CREATE: {
      const { key, kind, load } = read_load_create(f!);
      void (kind === LoadKind.Gltf ? load_model(key, load) : load_image(key, load));
      return;
    }
  }
  console.warn(`[kernel] signal 0x${cmd.toString(16)} with no one to hear it`);
}

CORE_BUILD.signal = on_signal;

/** Asks for a device and tells the kernel when one answers, and again after
 * one is lost. */
function gpu_boot(): void {
  const lost = () => {
    CORE_BUILD.send(0, Command.GPU_DELETE);
    gpu_boot();
    request_frame();
  };
  void acquire_device(lost).then((info) => {
    if ("reason" in info) {
      // The same end as a browser that cannot read the wire: nothing here is a
      // moment to recover from, so it is said once, in front of everything,
      // rather than left to a console nobody on that browser will open
      // (app/unsupported.tsx).
      console.error(`[gpu] ${info.detail}; nothing can be drawn`);
      declareUnsupported(info.reason);
      return;
    }
    CORE_BUILD.send(0, Command.GPU_CREATE, encode_gpu_create({ min_uniform_offset_alignment: info.align, timestamps: info.timestamps }));
    request_frame();
  }, (err) => console.error("[gpu] the device request failed; nothing can be drawn:", err));
}

/** Starts the kernel: a device to draw with, then the world it draws.
 * Called once by the app's bootstrap, so that what boots is ordered rather
 * than whichever module happened to be imported first. */
export function boot_kernel(): void {
  gpu_boot();
  CORE_BUILD.call(
    Command.WORLD_CREATE,
    encode_world_create({
      development: __IS_DEV__,
      handheld: HANDHELD,
      geolocation: "geolocation" in navigator,
      orientation: orientation_supported(),
      orientation_permission: orientation_needs_permission(),
    }),
    (result) => result,
  );
}
