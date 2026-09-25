// @pxl/web/platform/web/host.ts

/**
 * The kernel's host on the web: the module in WebAssembly, rung from the
 * page's frame, drawing through the page's WebGPU (kernel/). The shell asks
 * it for what it asks any host — to start, to hear a control's command, to
 * know what the pointer holds — and nothing of how. A device's build puts its
 * own host here.
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import type { u32 } from "@xpute/core/abi/word.ts";
import type { GroundChannelKey } from "@/catalog/cell/cube_type.spec.ts";
import { type ActiveTool, Command, encode_input_tracking, Tracking } from "@/kernel/abi/cmd.spec.ts";
import { CORE_BUILD } from "@/kernel/ipc.ts";
import { boot_kernel } from "@/kernel/kernel.ts";
import { kernel_hold } from "@/kernel/control/input.ts";
import { orientation_permit } from "@/kernel/control/sensor.ts";
import { missing_before_boot } from "@/kernel/render/gpu.ts";
import { request_frame, strobe } from "@/kernel/sched.ts";
import type { UnsupportedReason } from "@/app/store/unsupported-store.ts";

/** Starts the kernel, or says why this device cannot draw it: a browser that
 * cannot is told so instead of being started, since the kernel would
 * otherwise fetch its module, open a world and raise toasts about loading it,
 * under a notice saying none of it can be shown. */
export function host_boot(): UnsupportedReason | null {
  const missing = missing_before_boot();
  if (missing) return missing;
  boot_kernel();
  return null;
}

/** Sends one of the UI's commands, rung at once with no quota: what it asks
 * of the host is heard inside the call, including the sensors' close sent
 * after the strobe has stopped. */
export function host_command(cmd: Command, pkt?: U8Array): void {
  CORE_BUILD.send(0, cmd, pkt);
  CORE_BUILD.ring(0);
  request_frame();
}

/** The tracking button. Following needs the compass, so where iOS gates it,
 * the prompt runs here, inside the click, and the kernel hears the answer
 * before the press — the way a page's handler awaits `requestPermission` and
 * the browser only reports the outcome. */
export async function host_tracking(tracking: Tracking): Promise<void> {
  if (tracking === Tracking.Follow && !(await orientation_permit())) {
    console.error("[orientation] could not start the sensor.");
    return;
  }
  CORE_BUILD.send(0, Command.INPUT_TRACKING, encode_input_tracking({ tracking }));
  strobe.input();
  request_frame();
}

/** The sensor button: the compass's prompt, where iOS gates it, is raised
 * synchronously in the click — Safari consumes the gesture's activation, so
 * one reached after an await can be refused — and the rest follows. */
export function host_enable_sensors(): void {
  void orientation_permit();
  host_command(Command.SENSOR_ENABLE);
}

/** What the pointer holds, which crosses to the kernel with its events. */
export function host_hold(tool: ActiveTool, channel: GroundChannelKey | null, value: u32, tier: u32): void {
  kernel_hold(tool, channel, value, tier);
}
