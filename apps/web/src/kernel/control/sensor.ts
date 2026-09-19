// @pxl/web/kernel/control/sensor.ts

/**
 * The device's two sensors as the browser grants them
 * (crates/pxl/web/kernel/src/control/sensor): the watch the kernel asked for,
 * the events it is told about, and the permission each waits on.
 *
 * A reading is never taken at face value. A dead geolocation watch keeps
 * calling back, so it is cleared rather than forgotten; the compass is gated
 * behind a prompt on iOS, and that prompt has to be raised inside the gesture
 * that asked for it.
 */

import { Cmd } from "@/kernel/abi/cmd.ts";
import { CORE_BUILD, packet } from "@/kernel/ipc.ts";
import { request_frame } from "@/kernel/sched.ts";

/** crates/pxl/web/wasm/src/lib.rs's sensor numbers. */
export const GEOLOCATION = 0;
export const ORIENTATION = 1;

type IOSOrientationEvent = DeviceOrientationEvent & { webkitCompassHeading?: number };
type IOSOrientationEventCtor = typeof DeviceOrientationEvent & { requestPermission?: () => Promise<"granted" | "denied"> };

export function orientation_supported(): boolean {
  return typeof DeviceOrientationEvent !== "undefined";
}

export function orientation_needs_permission(): boolean {
  return orientation_supported() && typeof (DeviceOrientationEvent as IOSOrientationEventCtor).requestPermission === "function";
}

let geolocation_watch: number | null = null;

export function geolocation_stop(): void {
  if (geolocation_watch !== null) navigator.geolocation.clearWatch(geolocation_watch);
  geolocation_watch = null;
}

export function geolocation_start(): void {
  if (geolocation_watch !== null || !("geolocation" in navigator)) return;
  geolocation_watch = navigator.geolocation.watchPosition(
    (pos) => {
      const { latitude, longitude, altitude, accuracy, heading } = pos.coords;
      CORE_BUILD.send(0, Cmd.SENSOR_UPDATE, packet((b) => b.f64(longitude).f64(latitude).f64(altitude ?? NaN).f64(accuracy).f64(heading ?? NaN)));
      request_frame();
    },
    (err) => {
      // The dead watch is cleared, not just forgotten: it keeps calling back,
      // and a later start would race it.
      geolocation_stop();
      const code = err.code === err.PERMISSION_DENIED ? 1 : err.code === err.POSITION_UNAVAILABLE ? 2 : err.code === err.TIMEOUT ? 3 : 0;
      CORE_BUILD.send(0, Cmd.SENSOR_ERROR, packet((b) => b.u32(code)));
      request_frame();
    },
    { enableHighAccuracy: true, maximumAge: 5000, timeout: 30000 },
  );
}

let orientation_event: string | null = null;

function on_orientation(e: Event): void {
  const o = e as IOSOrientationEvent;
  CORE_BUILD.send(0, Cmd.SENSOR_ORIENTATION, packet((b) => b.f64(o.webkitCompassHeading ?? NaN).f64(o.alpha ?? NaN).f64(o.beta ?? NaN).f64(o.gamma ?? NaN)));
  request_frame();
}

function listen_orientation(name: string): void {
  if (orientation_event !== null) return;
  orientation_event = name;
  globalThis.addEventListener(name, on_orientation, true);
}

export function orientation_listen(): void {
  listen_orientation("ondeviceorientationabsolute" in globalThis ? "deviceorientationabsolute" : "deviceorientation");
}

/** crates/pxl/web/kernel/src/control/sensor/store.rs's PermissionAnswer. */
export const PERMISSION_ASKING = 0, PERMISSION_GRANTED = 1, PERMISSION_DENIED = 2, PERMISSION_FAILED = 3;

export function send_permission(sensor: number, answer: number): void {
  CORE_BUILD.send(0, Cmd.SENSOR_PERMISSION, packet((b) => b.u32(sensor).u32(answer)));
  request_frame();
}

/**
 * Whether the compass may be listened to, prompting where iOS gates it. The
 * prompt is called before anything is awaited, so it stays inside the gesture
 * that called this; a listener already running needs no prompt.
 */
export function orientation_permit(): Promise<boolean> {
  if (!orientation_needs_permission() || orientation_event !== null) return Promise.resolve(true);
  const prompt = (DeviceOrientationEvent as IOSOrientationEventCtor).requestPermission!();
  send_permission(ORIENTATION, PERMISSION_ASKING);
  return prompt.then(
    (permission) => {
      const granted = permission === "granted";
      if (granted) listen_orientation("deviceorientation");
      send_permission(ORIENTATION, granted ? PERMISSION_GRANTED : PERMISSION_DENIED);
      return granted;
    },
    (err) => {
      console.error("[kernel] orientation permission error:", err);
      send_permission(ORIENTATION, PERMISSION_FAILED);
      return false;
    },
  );
}

export function orientation_remove(): void {
  if (orientation_event !== null) globalThis.removeEventListener(orientation_event, on_orientation, true);
  orientation_event = null;
}
