// @pxl/web/sensor.ts

/**
 * The device's sensors as the browser grants them. iOS gates the compass
 * behind a prompt that must be raised inside the gesture.
 */

import { GeolocationErrorCode, PermissionAnswer, Sensor } from "@pxl/client/kernel/abi/cmd.spec.ts";
import { kernel_orientation, kernel_permission, kernel_position, kernel_position_error } from "@pxl/client/kernel/control/sensor.ts";

interface IosOrientationEvent extends DeviceOrientationEvent {
  webkitCompassHeading?: number;
}
type IosOrientationEventCtor = typeof DeviceOrientationEvent & { requestPermission?: () => Promise<"granted" | "denied"> };

export function geolocationSupported(): boolean {
  return "geolocation" in navigator;
}

export function orientationSupported(): boolean {
  return typeof DeviceOrientationEvent !== "undefined";
}

export function orientationNeedsPermission(): boolean {
  return orientationSupported() && typeof (DeviceOrientationEvent as IosOrientationEventCtor).requestPermission === "function";
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
      const { latitude, longitude, altitude, altitudeAccuracy, accuracy, heading } = pos.coords;
      // An altitude with no accuracy is none measured: a desktop's location gives 0.
      kernel_position(longitude, latitude, altitudeAccuracy === null ? null : altitude, accuracy, heading);
    },
    (err) => {
      // A watch keeps trying through POSITION_UNAVAILABLE and TIMEOUT and ends
      // only on PERMISSION_DENIED (https://www.w3.org/TR/geolocation/#watchposition-method),
      // so it is left running on any other; the kernel shows the failure until a fix lands.
      if (err.code === err.PERMISSION_DENIED) geolocationStop();
      const code = err.code === err.PERMISSION_DENIED
        ? GeolocationErrorCode.PermissionDenied
        : err.code === err.POSITION_UNAVAILABLE
        ? GeolocationErrorCode.PositionUnavailable
        : err.code === err.TIMEOUT
        ? GeolocationErrorCode.Timeout
        : GeolocationErrorCode.Other;
      kernel_position_error(code);
    },
    // No timeout: one would raise TIMEOUT on a clock while no fix comes
    // (PositionOptions.timeout, Infinity by default), each a turn for nothing.
    { enableHighAccuracy: true, maximumAge: 5000 },
  );
}

let orientationEvent: string | null = null;

function onOrientation(e: Event): void {
  const o = e as IosOrientationEvent;
  kernel_orientation(o.webkitCompassHeading ?? null, o.alpha, o.beta, o.gamma);
}

function listenOrientation(name: string): void {
  if (orientationEvent !== null) return;
  orientationEvent = name;
  globalThis.addEventListener(name, onOrientation, true);
}

export function orientationListen(): void {
  listenOrientation("ondeviceorientationabsolute" in globalThis ? "deviceorientationabsolute" : "deviceorientation");
}

/** A grant in the browser's site settings reaches the kernel, since nothing re-asks after a refusal. */
export function permissionFollow(): () => void {
  let status: PermissionStatus | null = null;
  let stopped = false;
  const onChange = () => kernel_permission(Sensor.Geolocation, status?.state === "granted" ? PermissionAnswer.Granted : PermissionAnswer.Denied);
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
  const prompt = (DeviceOrientationEvent as IosOrientationEventCtor).requestPermission!();
  kernel_permission(Sensor.Orientation, PermissionAnswer.Asking);
  return prompt.then(
    (permission) => {
      const granted = permission === "granted";
      if (granted) listenOrientation("deviceorientation");
      kernel_permission(Sensor.Orientation, granted ? PermissionAnswer.Granted : PermissionAnswer.Denied);
      return granted;
    },
    (err) => {
      console.error("[kernel] orientation permission error:", err);
      kernel_permission(Sensor.Orientation, PermissionAnswer.Failed);
      return false;
    },
  );
}

export function orientationRemove(): void {
  if (orientationEvent !== null) globalThis.removeEventListener(orientationEvent, onOrientation, true);
  orientationEvent = null;
}
