// @pxl/web/platform/web/sensor.ts

/**
 * The device's sensors as the browser grants them. iOS gates the compass
 * behind a prompt that must be raised inside the gesture.
 */

import { Command, encode_sensor_error, encode_sensor_orientation, encode_sensor_permission, encode_sensor_position, GeolocationErrorCode, PermissionAnswer, Sensor } from "@/kernel/abi/cmd.spec.ts";
import { CORE_DOORBELL } from "@/kernel/mem.ts";
import { request_frame } from "@/kernel/sched.ts";

interface IOSOrientationEvent extends DeviceOrientationEvent {
  webkitCompassHeading?: number;
}
type IOSOrientationEventCtor = typeof DeviceOrientationEvent & { requestPermission?: () => Promise<"granted" | "denied"> };

export function geolocationSupported(): boolean {
  return "geolocation" in navigator;
}

export function orientationSupported(): boolean {
  return typeof DeviceOrientationEvent !== "undefined";
}

export function orientationNeedsPermission(): boolean {
  return orientationSupported() && typeof (DeviceOrientationEvent as IOSOrientationEventCtor).requestPermission === "function";
}

/** No event says the listener stopped hearing, so silence is how a loss shows. */
export function orientationSilenceIsLoss(): boolean {
  return true;
}

let geolocationWatch: number | null = null;

export function geolocationStop(): void {
  if (geolocationWatch !== null) navigator.geolocation.clearWatch(geolocationWatch);
  geolocationWatch = null;
}

export function geolocationStart(): void {
  if (geolocationWatch !== null || !("geolocation" in navigator)) return;
  geolocationWatch = navigator.geolocation.watchPosition(
    (pos) => {
      const { latitude, longitude, altitude, accuracy, heading } = pos.coords;
      CORE_DOORBELL.send(0, Command.SENSOR_POSITION, encode_sensor_position({ lon: longitude, lat: latitude, altitude, accuracy, heading }));
      request_frame();
    },
    (err) => {
      // Cleared, not just forgotten: a dead watch keeps calling back and would race a later start.
      geolocationStop();
      const code = err.code === err.PERMISSION_DENIED
        ? GeolocationErrorCode.PermissionDenied
        : err.code === err.POSITION_UNAVAILABLE
        ? GeolocationErrorCode.PositionUnavailable
        : err.code === err.TIMEOUT
        ? GeolocationErrorCode.Timeout
        : GeolocationErrorCode.Other;
      CORE_DOORBELL.send(0, Command.SENSOR_ERROR, encode_sensor_error({ code }));
      request_frame();
    },
    { enableHighAccuracy: true, maximumAge: 5000, timeout: 30000 },
  );
}

let orientationEvent: string | null = null;

function onOrientation(e: Event): void {
  const o = e as IOSOrientationEvent;
  CORE_DOORBELL.send(0, Command.SENSOR_ORIENTATION, encode_sensor_orientation({ webkit_compass_heading: o.webkitCompassHeading ?? null, alpha: o.alpha, beta: o.beta, gamma: o.gamma }));
  request_frame();
}

function listenOrientation(name: string): void {
  if (orientationEvent !== null) return;
  orientationEvent = name;
  globalThis.addEventListener(name, onOrientation, true);
}

export function orientationListen(): void {
  listenOrientation("ondeviceorientationabsolute" in globalThis ? "deviceorientationabsolute" : "deviceorientation");
}

function sendPermission(sensor: Sensor, answer: PermissionAnswer): void {
  CORE_DOORBELL.send(0, Command.SENSOR_PERMISSION, encode_sensor_permission({ sensor, answer }));
  request_frame();
}

/** A grant in the browser's site settings reaches the kernel, since nothing re-asks after a refusal. */
export function permissionFollow(): () => void {
  let status: PermissionStatus | null = null;
  let stopped = false;
  const onChange = () => sendPermission(Sensor.Geolocation, status?.state === "granted" ? PermissionAnswer.Granted : PermissionAnswer.Denied);
  navigator.permissions?.query({ name: "geolocation" }).then((s) => {
    if (stopped) return;
    status = s;
    s.addEventListener("change", onChange);
  }).catch(() => {});
  return () => {
    stopped = true;
    status?.removeEventListener("change", onChange);
  };
}

/** The prompt is called before any await, so it stays inside the gesture. */
export function orientationPermit(): Promise<boolean> {
  if (!orientationNeedsPermission() || orientationEvent !== null) return Promise.resolve(true);
  const prompt = (DeviceOrientationEvent as IOSOrientationEventCtor).requestPermission!();
  sendPermission(Sensor.Orientation, PermissionAnswer.Asking);
  return prompt.then(
    (permission) => {
      const granted = permission === "granted";
      if (granted) listenOrientation("deviceorientation");
      sendPermission(Sensor.Orientation, granted ? PermissionAnswer.Granted : PermissionAnswer.Denied);
      return granted;
    },
    (err) => {
      console.error("[kernel] orientation permission error:", err);
      sendPermission(Sensor.Orientation, PermissionAnswer.Failed);
      return false;
    },
  );
}

export function orientationRemove(): void {
  if (orientationEvent !== null) globalThis.removeEventListener(orientationEvent, onOrientation, true);
  orientationEvent = null;
}
