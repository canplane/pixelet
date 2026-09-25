// @pxl/web/kernel/shell.ts

/**
 * What the page's UI reads of the kernel and what it asks of it
 * (crates/pxl/web/kernel/src/shell/shell.rs): one snapshot it renders, and
 * the commands its controls send.
 *
 * The snapshot arrives as a Snapshot packet in a block the kernel holds, read
 * whole when it is signalled and replaced whole — a component subscribes and
 * re-renders on the part it selects. A control's command is rung at once with
 * no quota, so what it asks of the host is heard inside the call.
 */

import { useSyncExternalStore } from "react";

import type { U8Array } from "@xpute/core/abi/array.ts";
import { TreeReader } from "@xpute/core/wire/xtp/mod.ts";
import { Command, DevicePoseStatus, encode_input_tracking, MapStatus, read_snapshot, type Snapshot, Tracking } from "@/kernel/abi/cmd.spec.ts";
import { CORE_BUILD } from "@/kernel/ipc.ts";
import { orientation_permit } from "@/kernel/control/sensor.ts";
import { request_frame, strobe } from "@/kernel/sched.ts";

const INITIAL: Snapshot = {
  map: { status: MapStatus.Idle, is_off_center: false, center: null, page: null },
  location: { status: DevicePoseStatus.Idle, has_location: false, error: null },
  companion: { person_ready: false, pet_ready: false },
  orientation: { status: DevicePoseStatus.Idle, heading: null },
  azimuth: 0,
  is_follow_mode: false,
  camera_busy: false,
  show_compass: false,
  settings: [],
  scale_bar: { total_meters: 0, bar_width_px: 0, is_default_zoom: true },
  place: [],
  object_sizes: [],
};

let snapshot: Snapshot = INITIAL;
const snapshot_listeners = new Set<() => void>();

/** Reads the kernel's snapshot out of the bytes it holds it in: all of it,
 * now, since the kernel writes the next one there. */
export function set_snapshot(bytes: U8Array): void {
  snapshot = read_snapshot(new TreeReader(bytes).read_branch());
  for (const listener of snapshot_listeners) listener();
}

function subscribe_snapshot(listener: () => void): () => void {
  snapshot_listeners.add(listener);
  return () => snapshot_listeners.delete(listener);
}

export function kernel_snapshot(): Snapshot {
  return snapshot;
}

/** The part of the kernel's snapshot a component shows. */
export function useKernel<T>(select: (s: Snapshot) => T): T {
  return select(useSyncExternalStore(subscribe_snapshot, kernel_snapshot));
}

/** Sends one of the UI's commands, rung at once with no quota: what it asks
 * of the host is heard inside the call, including the sensors' close sent
 * after the strobe has stopped. */
export function kernel_command(cmd: Command, pkt?: U8Array): void {
  CORE_BUILD.send(0, cmd, pkt);
  CORE_BUILD.ring(0);
  request_frame();
}

/** The tracking button. Following needs the compass, so where iOS gates it,
 * the prompt runs here, inside the click, and the kernel hears the answer
 * before the press — the way a page's handler awaits `requestPermission` and
 * the browser only reports the outcome. */
export async function kernel_tracking(tracking: Tracking): Promise<void> {
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
export function kernel_enable_sensors(): void {
  void orientation_permit();
  kernel_command(Command.SENSOR_ENABLE);
}
