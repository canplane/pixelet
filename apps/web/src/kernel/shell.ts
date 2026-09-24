// @pxl/web/kernel/shell.ts

/**
 * What the page's UI reads of the kernel and what it asks of it
 * (crates/pxl/web/kernel/src/shell/shell.rs): one snapshot it renders, and
 * the actions it applies.
 *
 * The snapshot arrives as JSON in a block the kernel holds, and is replaced
 * whole — a component subscribes and re-renders on the part it selects. An
 * action is rung at once with no quota, so what it asks of the host is heard
 * inside the call.
 */

import { useSyncExternalStore } from "react";

import type { u32 } from "@xpute/core/abi/word.ts";
import { Command } from "@/kernel/abi/cmd.spec.ts";
import type { PlacePart } from "@/feature/map/geocode/label.ts";
import type { Message } from "@/kernel/abi/message.ts";
import { CORE_BUILD, packet } from "@/kernel/ipc.ts";
import { orientation_permit } from "@/kernel/control/sensor.ts";
import { request_frame, strobe } from "@/kernel/sched.ts";

export type PoseStatus = "idle" | "loading" | "ready" | "error";

/** A badge on the map, as the panel beside it lists it: `id` is the cell it
 * stands for, handed back to go there (`poi_focus`); `name` is the one place
 * it holds, where it holds one. */
export interface PoiBadge {
  id: string;
  count: number;
  name: string | null;
}
export type UiSnapshot = {
  /** `center` is the map centre as [lon, lat]; `page` is the L3 page it falls
   * in, as a decimal string because a page index is 64-bit. The kernel sends
   * both because it has both — recomputing the page here would mean a second
   * implementation of the space in TypeScript. */
  map: { status: "idle" | "loading" | "success" | "error"; is_off_center: boolean; center: [number, number] | null; page: string | null };
  location: { status: PoseStatus; has_location: boolean; error: Message | null };
  companion: { person_ready: boolean; pet_ready: boolean };
  orientation: { status: PoseStatus; heading: number | null };
  view: { azimuth: number };
  is_follow_mode: boolean;
  camera_busy: boolean;
  settings: Record<string, boolean | string | number>;
  scale_bar: { total_meters: number; bar_width_px: number; is_default_zoom: boolean };
  /** The place the map centre is in, broad to narrow, already cut at the
   * rung the current zoom asks for; empty where the centre is in none or the
   * packets it is read out of have not arrived. */
  place: PlacePart[];
  /** The badges on the map, nearest what the view looks at first. */
  poi: PoiBadge[];
  compass: { show: boolean };
  /** The sizes an object places at, in units, coarsest first: the tiers the
   * size flyout offers, one of which the shell holds by its index. */
  object_sizes: number[];
  object_storage_version: number;
};

const INITIAL: UiSnapshot = {
  map: { status: "idle", is_off_center: false, center: null, page: null },
  location: { status: "idle", has_location: false, error: null },
  companion: { person_ready: false, pet_ready: false },
  orientation: { status: "idle", heading: null },
  view: { azimuth: 0 },
  is_follow_mode: false,
  camera_busy: false,
  settings: {},
  poi: [],
  scale_bar: { total_meters: 0, bar_width_px: 0, is_default_zoom: true },
  place: [],
  compass: { show: false },
  object_sizes: [],
  object_storage_version: 0,
};

let snapshot_text = "";
let snapshot: UiSnapshot = INITIAL;
const snapshot_listeners = new Set<() => void>();

export function set_snapshot(text: string): void {
  if (text === snapshot_text) return;
  snapshot_text = text;
  snapshot = JSON.parse(text) as UiSnapshot;
  for (const listener of snapshot_listeners) listener();
}

function subscribe_snapshot(listener: () => void): () => void {
  snapshot_listeners.add(listener);
  return () => snapshot_listeners.delete(listener);
}

export function kernel_snapshot(): UiSnapshot {
  return snapshot;
}

/** The part of the kernel's snapshot a component shows. */
export function useKernel<T>(select: (s: UiSnapshot) => T): T {
  return select(useSyncExternalStore(subscribe_snapshot, kernel_snapshot));
}

export interface KernelAction {
  type: string;
  [key: string]: unknown;
}

/** Applies one UI action in the kernel, rung at once with no quota: what it
 * asks of the host is heard inside the call, including the sensors' close sent
 * after the strobe has stopped. */
export function kernel_action(action: KernelAction): void {
  CORE_BUILD.send(0, Command.WORLD_ACTION, packet((b) => b.str(JSON.stringify(action))));
  CORE_BUILD.ring(0);
  request_frame();
}

/** The tracking button's press: crates/pxl/web/kernel/src/shell/shell.rs's Tracking. */
export const TRACKING_RECENTER = 0, TRACKING_FOLLOW = 1, TRACKING_UNFOLLOW = 2;

/** The tracking button. Following needs the compass, so where iOS gates it,
 * the prompt runs here, inside the click, and the kernel hears the answer
 * before the press — the way a page's handler awaits `requestPermission` and
 * the browser only reports the outcome. */
export async function kernel_tracking(tracking: u32): Promise<void> {
  if (tracking === TRACKING_FOLLOW && !(await orientation_permit())) {
    console.error("[orientation] could not start the sensor.");
    return;
  }
  CORE_BUILD.send(0, Command.INPUT_TRACKING, packet((b) => b.u32(tracking)));
  strobe.input();
  request_frame();
}

/** The sensor button: the compass's prompt, where iOS gates it, is raised
 * synchronously in the click — Safari consumes the gesture's activation, so
 * one reached after an await can be refused — and the rest follows. */
export function kernel_enable_sensors(): void {
  void orientation_permit();
  kernel_action({ type: "enable_sensors" });
}
