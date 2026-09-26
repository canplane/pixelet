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

import { Command, encode_sensor_error, encode_sensor_orientation, encode_sensor_permission, encode_sensor_update, GeolocationErrorCode, PermissionAnswer, Sensor } from "@/kernel/abi/cmd.spec.ts";
import { CORE_BUILD } from "@/kernel/ipc.ts";
import { request_frame } from "@/kernel/sched.ts";

interface IOSOrientationEvent extends DeviceOrientationEvent {
  webkitCompassHeading?: number;
}
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
      CORE_BUILD.send(0, Command.SENSOR_UPDATE, encode_sensor_update({ lon: longitude, lat: latitude, altitude, accuracy, heading }));
      request_frame();
    },
    (err) => {
      // The dead watch is cleared, not just forgotten: it keeps calling back,
      // and a later start would race it.
      geolocation_stop();
      const code = err.code === err.PERMISSION_DENIED
        ? GeolocationErrorCode.PermissionDenied
        : err.code === err.POSITION_UNAVAILABLE
        ? GeolocationErrorCode.PositionUnavailable
        : err.code === err.TIMEOUT
        ? GeolocationErrorCode.Timeout
        : GeolocationErrorCode.Other;
      CORE_BUILD.send(0, Command.SENSOR_ERROR, encode_sensor_error({ code }));
      request_frame();
    },
    { enableHighAccuracy: true, maximumAge: 5000, timeout: 30000 },
  );
}

let orientation_event: string | null = null;

function on_orientation(e: Event): void {
  const o = e as IOSOrientationEvent;
  CORE_BUILD.send(0, Command.SENSOR_ORIENTATION, encode_sensor_orientation({ webkit_compass_heading: o.webkitCompassHeading ?? null, alpha: o.alpha, beta: o.beta, gamma: o.gamma }));
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

export function send_permission(sensor: Sensor, answer: PermissionAnswer): void {
  CORE_BUILD.send(0, Command.SENSOR_PERMISSION, encode_sensor_permission({ sensor, answer }));
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
  send_permission(Sensor.Orientation, PermissionAnswer.Asking);
  return prompt.then(
    (permission) => {
      const granted = permission === "granted";
      if (granted) listen_orientation("deviceorientation");
      send_permission(Sensor.Orientation, granted ? PermissionAnswer.Granted : PermissionAnswer.Denied);
      return granted;
    },
    (err) => {
      console.error("[kernel] orientation permission error:", err);
      send_permission(Sensor.Orientation, PermissionAnswer.Failed);
      return false;
    },
  );
}

export function orientation_remove(): void {
  if (orientation_event !== null) globalThis.removeEventListener(orientation_event, on_orientation, true);
  orientation_event = null;
}
