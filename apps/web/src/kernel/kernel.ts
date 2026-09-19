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
import { Cmd } from "@/kernel/abi/cmd.ts";
import { APP_ENV, BUILD_ID, BUILD_NUMBER, BUILD_TIME, PUBLISHED_TILES } from "@/config/deploy.ts";
import { HANDHELD } from "@/shared/lib/browser/handheld.ts";
import { GEOLOCATION, geolocation_start, geolocation_stop, orientation_listen, orientation_needs_permission, orientation_remove, orientation_supported } from "@/kernel/control/sensor.ts";
import { abort_fetch, run_fetch } from "@/kernel/io/fetch.ts";
import { load_image, load_model } from "@/kernel/io/load.ts";
import { CORE_BUILD, packet } from "@/kernel/ipc.ts";
import { CORE_KERNEL } from "@/kernel/mem.ts";
import { text_at } from "@/kernel/module.ts";
import { acquire_device } from "@/kernel/render/gpu.ts";
import { request_frame } from "@/kernel/sched.ts";
import { set_snapshot } from "@/kernel/shell.ts";

function on_signal(_tag: u32, cmd: Cmd, f: BranchCursor | null): void {
  switch (cmd) {
    case Cmd.WORLD_SNAPSHOT: {
      // The JSON is far past a ring frame, so the kernel names the block it
      // holds it in. It is the kernel's and stays valid until it makes the
      // next one, which is a turn away.
      const [at, len] = f!.get<[number, number]>();
      set_snapshot(text_at(CORE_KERNEL.memory, at >>> 0, len >>> 0));
      return;
    }
    case Cmd.WORLD_UPDATE:
      return;
    case Cmd.SENSOR_CREATE:
      if (f!.at(0).get<number>() === GEOLOCATION) geolocation_start();
      else orientation_listen();
      return;
    case Cmd.SENSOR_DELETE:
      if (f!.at(0).get<number>() === GEOLOCATION) geolocation_stop();
      else orientation_remove();
      return;
    case Cmd.FETCH_CREATE: {
      const [key, url] = f!.get<[bigint, string]>();
      void run_fetch(key, url);
      return;
    }
    case Cmd.FETCH_DELETE:
      abort_fetch(f!.at(0).get<bigint>());
      return;
    case Cmd.LOAD_CREATE: {
      const [kind, load] = f!.get<[number, number]>();
      void (kind === 0 ? load_model(load) : load_image(load));
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
    CORE_BUILD.send(0, Cmd.GPU_DELETE);
    gpu_boot();
    request_frame();
  };
  void acquire_device(lost).then((info) => {
    if ("reason" in info) {
      console.error(`[gpu] ${info.reason}; nothing can be drawn`);
      return;
    }
    CORE_BUILD.send(0, Cmd.GPU_CREATE, packet((b) => b.u32(info.align).bool(info.timestamps)));
    request_frame();
  }, (err) => console.error("[gpu] the device request failed; nothing can be drawn:", err));
}

/** Starts the kernel: a device to draw with, then the world it draws.
 * Called once by the app's bootstrap, so that what boots is ordered rather
 * than whichever module happened to be imported first. */
export function boot_kernel(): void {
  gpu_boot();
  CORE_BUILD.call(
    Cmd.WORLD_CREATE,
    packet((b) =>
      b.str(APP_ENV).str(BUILD_TIME).str(BUILD_ID).str(BUILD_NUMBER).bool(PUBLISHED_TILES).bool(HANDHELD)
        .bool("geolocation" in navigator).bool(orientation_supported()).bool(orientation_needs_permission())
    ),
    (result) => result,
  );
}
