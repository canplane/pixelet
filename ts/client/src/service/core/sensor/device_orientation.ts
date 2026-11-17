// @pxl/client/service/core/sensor/device_orientation.ts

/** The compass sensor's decisions: when to start it, and when silence counts
 * as a lost sensor. The platform owns the listener, the permission and the
 * heading an event means, and the driver hears the heading. */

import type { f64 } from "@xpute/kit/abi/word.ts";
import { WATCHDOG_TIMEOUT_MS } from "./param.ts";
import {
  deviceOrientationStore,
  setIsTrackingOrientation,
  setOrientation,
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
  /** Whether silence means the sensor is lost. A browser's sensor talks even
   * while the device is still. A native sensor (Core Location) speaks only
   * on change and reports its own failure. */
  silenceIsLoss: boolean;
}

/** The compass as the platform reports it once the client starts. Until then
 * the place has none. */
const PLATFORM: OrientationPlatform = {
  supported: false,
  silenceIsLoss: false,
};

export function platform(): OrientationPlatform {
  return PLATFORM;
}

interface Orientation {
  /** The time the watchdog measures silence from: the last event or the start
   * of tracking. It is read only while tracking, and every start of tracking
   * sets it. */
  lastEventAt: number;
  /** The last command. The platform's listener follows it and ignores a
   * repeated command. */
  command: OrientationCommand | null;
}

const orientation: Orientation = { lastEventAt: 0, command: null };

/** A heading, in degrees clockwise from north, or null for a reading with no
 * heading. */
export function orientationEvent(heading: f64 | null): void {
  orientation.lastEventAt = performance.now();
  setOrientation(deviceOrientationStore(), heading);
}

/** The place has no compass, as a web event with every field null says: its
 * listener stops, and the compass reads unavailable, which `requestOrientation`
 * keeps until the world's surface closes. */
export function orientationUnavailable(): void {
  orientation.command = ORIENTATION_COMMAND_STOP;
  const store = deviceOrientationStore();
  setIsTrackingOrientation(store, false);
  store.status = SENSOR_STATUS_UNAVAILABLE;
}

/** Asks for the heading, once the platform permits it. Returns true when the
 * heading is tracked or now asked for. */
export function requestOrientation(): boolean {
  const store = deviceOrientationStore();
  if (store.isTrackingOrientation) return true;
  if (!PLATFORM.supported || store.status === SENSOR_STATUS_UNAVAILABLE) {
    store.status = SENSOR_STATUS_UNAVAILABLE;
    return false;
  }
  store.status = SENSOR_STATUS_LOADING;
  orientation.command = ORIENTATION_COMMAND_START;
  orientation.lastEventAt = performance.now();
  setIsTrackingOrientation(store, true);
  return true;
}

/** The compass is lost, or its permission refused. */
export function onOrientationLost(): void {
  const store = deviceOrientationStore();
  setIsTrackingOrientation(store, false);
  store.status = SENSOR_STATUS_ERROR;
}

export function orientationClose(): void {
  orientation.command = ORIENTATION_COMMAND_STOP;
  const store = deviceOrientationStore();
  setIsTrackingOrientation(store, false);
  store.status = SENSOR_STATUS_IDLE;
}

/** Stops a tracked heading while the app is hidden, its status kept; the
 * return asks for it again (`requestOrientation`), whose watchdog measures afresh
 * from then. */
export function orientationStop(): void {
  const store = deviceOrientationStore();
  if (store.isTrackingOrientation) {
    orientation.command = ORIENTATION_COMMAND_STOP;
    setIsTrackingOrientation(store, false);
  }
}

/** Judges whether the heading is lost, and returns when to judge again, or
 * null. It is armed only while a heading is tracked on a platform where
 * silence is the only sign of loss, so nothing is woken while nothing is
 * tracked. It wakes once, when the silence since the last event would amount
 * to a loss. An event before then moves that deadline, so a sensor that keeps
 * talking is never polled. */
export function orientationWatchdogStep(): number | null {
  if (
    !PLATFORM.silenceIsLoss ||
    !deviceOrientationStore().isTrackingOrientation
  )
    return null;
  const lostAt = orientation.lastEventAt + WATCHDOG_TIMEOUT_MS;
  if (performance.now() < lostAt) return lostAt;
  orientation.command = ORIENTATION_COMMAND_STOP;
  onOrientationLost();
  return null;
}

export function takeOrientationCommand(): OrientationCommand | null {
  const c = orientation.command;
  orientation.command = null;
  return c;
}
