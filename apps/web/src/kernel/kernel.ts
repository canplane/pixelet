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

import type { u32, u64 } from "@xpute/core/abi/word.ts";
import type { BranchCursor } from "@xpute/core/wire/xtp/mod.ts";
import { Command } from "@/kernel/abi/cmd.spec.ts";
import { APP_ENV, BUILD_ID, BUILD_NUMBER, BUILD_TIME, PUBLISHED_TILES } from "@/config/deploy.ts";
import { HANDHELD } from "@/shared/lib/browser/handheld.ts";
import { GEOLOCATION, geolocation_start, geolocation_stop, orientation_listen, orientation_needs_permission, orientation_remove, orientation_supported } from "@/kernel/control/sensor.ts";
import { abort_fetch, run_fetch } from "@/kernel/io/fetch.ts";
import { load_image, load_model } from "@/kernel/io/load.ts";
import { CORE_BUILD, packet } from "@/kernel/ipc.ts";
import { CORE_KERNEL } from "@/kernel/mem.ts";
import { text_at } from "@/kernel/module.ts";
import { declareUnsupported } from "@/app/store/unsupported-store.ts";
import { acquire_device } from "@/kernel/render/gpu.ts";
import { request_frame } from "@/kernel/sched.ts";
import { set_snapshot } from "@/kernel/shell.ts";

function on_signal(_tag: u32, cmd: Command, f: BranchCursor | null): void {
  switch (cmd) {
    case Command.WORLD_SNAPSHOT: {
      // The JSON is far past a ring frame, so the kernel names the block it
      // holds it in. It is the kernel's and stays valid until it makes the
      // next one, which is a turn away.
      const [at, len] = f!.get<[u32, u32]>();
      set_snapshot(text_at(CORE_KERNEL.memory, at >>> 0, len >>> 0));
      return;
    }
    case Command.WORLD_UPDATE:
      return;
    case Command.SENSOR_CREATE:
      if (f!.at(0).get<u32>() === GEOLOCATION) geolocation_start();
      else orientation_listen();
      return;
    case Command.SENSOR_DELETE:
      if (f!.at(0).get<u32>() === GEOLOCATION) geolocation_stop();
      else orientation_remove();
      return;
    case Command.FETCH_CREATE: {
      const [key, bake, name] = f!.get<[u64, u32, string]>();
      void run_fetch(key, bake, name);
      return;
    }
    case Command.FETCH_DELETE:
      abort_fetch(f!.at(0).get<u64>());
      return;
    case Command.LOAD_CREATE: {
      const [key, kind, load] = f!.get<[u64, u32, u32]>();
      void (kind === 0 ? load_model(key, load) : load_image(key, load));
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
    CORE_BUILD.send(0, Command.GPU_CREATE, packet((b) => b.u32(info.align).bool(info.timestamps)));
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
    packet((b) =>
      b.str(APP_ENV).str(BUILD_TIME).str(BUILD_ID).str(BUILD_NUMBER).bool(PUBLISHED_TILES).bool(HANDHELD)
        .bool("geolocation" in navigator).bool(orientation_supported()).bool(orientation_needs_permission())
    ),
    (result) => result,
  );
}
