// @pxl/client/service/core/sensor/device_orientation.ts

/** The compass sensor's decisions: the heading an event means, when to start
 * it, and when silence counts as a lost sensor. The platform owns the listener
 * and iOS's permission prompt, and the driver hears only the answer. */

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { PermissionAnswer } from "@/service/platform/types.ts";
import { WATCHDOG_TIMEOUT_MS } from "./param.ts";
import {
  device_orient_store,
  set_is_tracking_orient,
  set_orient,
  SENSOR_STATUS_ERROR,
  SENSOR_STATUS_IDLE,
  SENSOR_STATUS_LOADING,
  SENSOR_STATUS_UNAVAILABLE,
} from "./reading.ts";

export type OrientationCommand =
  typeof ORIENTATION_COMMAND_START | typeof ORIENTATION_COMMAND_STOP;
/** The platform starts the compass, deviceorientationabsolute where the
 * browser has it. */
export const ORIENTATION_COMMAND_START = 0;
export const ORIENTATION_COMMAND_STOP = 1;

export interface OrientationPlatform {
  supported: boolean;
  /** On iOS 13 and later, `requestPermission` must run inside a user
   * gesture. */
  needs_permission: boolean;
  /** Whether silence means the sensor is lost. A browser's sensor talks even
   * while the device is still. A device's sensor (Core Location) speaks only
   * on change and reports its own failure. */
  silence_is_loss: boolean;
}

/** The compass as the platform reports it once the client starts. Until then
 * the place has none. */
const PLATFORM: OrientationPlatform = {
  supported: false,
  needs_permission: false,
  silence_is_loss: false,
};

export function platform(): OrientationPlatform {
  return PLATFORM;
}

interface Orientation {
  /** The time the watchdog measures silence from: the last event or the start
   * of tracking. It is read only while tracking, and every start of tracking
   * sets it. */
  last_event_at: number;
  /** The last command. The platform's listener follows it and ignores a
   * repeated command. */
  cmd: OrientationCommand | null;
}

const orient: Orientation = { last_event_at: 0, cmd: null };

/** An orientation event. iOS's compass heading is preferred, and otherwise
 * the heading is derived from alpha. */
export function orient_event(
  webkit_compass_heading: f64 | null,
  alpha: f64 | null,
): void {
  // An event with neither is a reading with no heading. One with no field at
  // all is a page's word that it has no compass, which its reader tells apart
  // (`orient_unavailable`).
  const heading =
    webkit_compass_heading ?? (alpha === null ? null : (360 - alpha) % 360);
  orient.last_event_at = performance.now();
  set_orient(device_orient_store(), heading);
}

/** The place has no compass, as a page's event with every field null says: its
 * listener stops, and the compass reads unavailable, which `request_orient`
 * keeps until the world's surface closes. */
export function orient_unavailable(): void {
  orient.cmd = ORIENTATION_COMMAND_STOP;
  const store = device_orient_store();
  set_is_tracking_orient(store, false);
  store.status = SENSOR_STATUS_UNAVAILABLE;
}

/** Asks for the heading. Returns true when the heading is tracked or now
 * asked for. Behind a permission, only the platform's prompt can start it, so
 * this returns false there. */
export function request_orient(): boolean {
  const store = device_orient_store();
  if (store.is_tracking_orient) return true;
  if (!PLATFORM.supported || store.status === SENSOR_STATUS_UNAVAILABLE) {
    store.status = SENSOR_STATUS_UNAVAILABLE;
    return false;
  }
  if (PLATFORM.needs_permission) return false;
  store.status = SENSOR_STATUS_LOADING;
  orient.cmd = ORIENTATION_COMMAND_START;
  orient.last_event_at = performance.now();
  set_is_tracking_orient(store, true);
  return true;
}

/** The answer to the orientation permission. Where no prompt gates the
 * sensor, a grant came from outside the app (the device's settings) and
 * starts the listener here. */
export function orient_permission(answer: PermissionAnswer): void {
  const store = device_orient_store();
  switch (answer) {
    case "asking":
      if (!store.is_tracking_orient) store.status = SENSOR_STATUS_LOADING;
      break;
    case "granted":
      if (PLATFORM.needs_permission) {
        orient.last_event_at = performance.now();
        set_is_tracking_orient(store, true);
      } else request_orient();
      break;
    case "denied":
    case "failed":
      set_is_tracking_orient(store, false);
      store.status = SENSOR_STATUS_ERROR;
  }
}

export function on_orient_lost(): void {
  const store = device_orient_store();
  set_is_tracking_orient(store, false);
  store.status = SENSOR_STATUS_ERROR;
}

export function orient_close(): void {
  orient.cmd = ORIENTATION_COMMAND_STOP;
  const store = device_orient_store();
  set_is_tracking_orient(store, false);
  store.status = SENSOR_STATUS_IDLE;
}

/** Stops a tracked heading while the app is hidden, its status kept; the
 * return asks for it again (`request_orient`), whose watchdog measures afresh
 * from then. */
export function orient_stop(): void {
  const store = device_orient_store();
  if (store.is_tracking_orient) {
    orient.cmd = ORIENTATION_COMMAND_STOP;
    set_is_tracking_orient(store, false);
  }
}

/** Judges whether the heading is lost, and returns when to judge again, or
 * null. It is armed only while a heading is tracked on a platform where
 * silence is the only sign of loss, so nothing is woken while nothing is
 * tracked. It wakes once, when the silence since the last event would amount
 * to a loss. An event before then moves that deadline, so a sensor that keeps
 * talking is never polled. */
export function orient_watchdog_step(): number | null {
  if (!PLATFORM.silence_is_loss || !device_orient_store().is_tracking_orient)
    return null;
  const lost_at = orient.last_event_at + WATCHDOG_TIMEOUT_MS;
  if (performance.now() < lost_at) return lost_at;
  orient.cmd = ORIENTATION_COMMAND_STOP;
  on_orient_lost();
  return null;
}

export function take_orient_cmd(): OrientationCommand | null {
  const c = orient.cmd;
  orient.cmd = null;
  return c;
}
