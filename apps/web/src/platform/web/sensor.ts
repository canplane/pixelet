// @pxl/web/platform/web/sensor.ts

/** The device's sensors as the browser grants them. */

import { GeolocationErrorCode, PermissionAnswer, Sensor } from "@/kernel/abi/cmd.spec.ts";
import { kernel_permission, kernel_position, kernel_position_error } from "@/kernel/control/sensor.ts";

export function geolocationSupported(): boolean {
  return "geolocation" in navigator;
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
      kernel_position({ lon: longitude, lat: latitude, altitude, accuracy, course: heading });
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
      kernel_position_error({ code });
    },
    { enableHighAccuracy: true, maximumAge: 5000, timeout: 30000 },
  );
}

/** A grant in the browser's site settings reaches the kernel, since nothing re-asks after a refusal. */
export function permissionFollow(): () => void {
  let status: PermissionStatus | null = null;
  let stopped = false;
  const onChange = () => kernel_permission({ sensor: Sensor.Geolocation, answer: status?.state === "granted" ? PermissionAnswer.Granted : PermissionAnswer.Denied });
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
