// @pxl/web/sensor.ts

/**
 * The device's sensors as the browser grants them. iOS gates the compass
 * behind a prompt that must be raised inside the gesture.
 */

import type { SensorHeard } from "@pxl/client/bracket.tsx";

/** Set once at boot, before any sensor starts. */
let heard: SensorHeard;

export function hear(given: SensorHeard): void {
  heard = given;
}

interface IosOrientationEvent extends DeviceOrientationEvent {
  webkitCompassHeading?: number;
}
type IosOrientationEventCtor = typeof DeviceOrientationEvent & { requestPermission?: () => Promise<"granted" | "denied"> };

function geo_has(): boolean {
  return "geolocation" in navigator;
}

function orient_has(): boolean {
  return typeof DeviceOrientationEvent !== "undefined";
}

function orient_gated(): boolean {
  return orient_has() && typeof (DeviceOrientationEvent as IosOrientationEventCtor).requestPermission === "function";
}

/** No event says the listener stopped hearing, so silence is how a loss shows. */
function orient_silence(): boolean {
  return true;
}

let geo_watch: number | null = null;

function geo_stop(): void {
  if (geo_watch !== null) navigator.geolocation.clearWatch(geo_watch);
  geo_watch = null;
}

function geo_start(): void {
  if (geo_watch !== null || !("geolocation" in navigator)) return;
  geo_watch = navigator.geolocation.watchPosition(
    (pos) => {
      const { latitude, longitude, altitude, altitudeAccuracy, accuracy, heading } = pos.coords;
      // An altitude with no accuracy is none measured: a desktop's location gives 0.
      heard.position(longitude, latitude, altitudeAccuracy === null ? null : altitude, accuracy, heading);
    },
    (err) => {
      // A watch keeps trying through POSITION_UNAVAILABLE and TIMEOUT and ends
      // only on PERMISSION_DENIED (https://www.w3.org/TR/geolocation/#watchposition-method),
      // so it is left running on any other; the kernel shows the failure until a fix lands.
      if (err.code === err.PERMISSION_DENIED) geo_stop();
      heard.positionError(
        err.code === err.PERMISSION_DENIED ? "denied" : err.code === err.POSITION_UNAVAILABLE ? "unavailable" : err.code === err.TIMEOUT ? "timeout" : "other",
      );
    },
    // No timeout: one would raise TIMEOUT on a clock while no fix comes
    // (PositionOptions.timeout, Infinity by default), each a turn for nothing.
    { enableHighAccuracy: true, maximumAge: 5000 },
  );
}

let orient_event: string | null = null;

function on_orient(e: Event): void {
  const o = e as IosOrientationEvent;
  heard.orientation(o.webkitCompassHeading ?? null, o.alpha, o.beta, o.gamma);
}

function orient_listen(name: string): void {
  if (orient_event !== null) return;
  orient_event = name;
  globalThis.addEventListener(name, on_orient, true);
}

function orient_start(): void {
  orient_listen("ondeviceorientationabsolute" in globalThis ? "deviceorientationabsolute" : "deviceorientation");
}

/** A grant in the browser's site settings reaches the kernel, since nothing re-asks after a refusal. */
export function follow(): () => void {
  let status: PermissionStatus | null = null;
  let stopped = false;
  const on_change = () => heard.permission("geolocation", status?.state === "granted" ? "granted" : "denied");
  navigator.permissions?.query({ name: "geolocation" }).then((s) => {
    if (stopped) return;
    status = s;
    s.addEventListener("change", on_change);
  }).catch(() => {});
  return () => {
    stopped = true;
    status?.removeEventListener("change", on_change);
  };
}

/** The prompt is called before any await, so it stays inside the gesture. */
function orient_permit(): Promise<boolean> {
  if (!orient_gated() || orient_event !== null) return Promise.resolve(true);
  const prompt = (DeviceOrientationEvent as IosOrientationEventCtor).requestPermission!();
  heard.permission("orientation", "asking");
  return prompt.then(
    (permission) => {
      const granted = permission === "granted";
      if (granted) orient_listen("deviceorientation");
      heard.permission("orientation", granted ? "granted" : "denied");
      return granted;
    },
    (err) => {
      console.error("[sensor] orientation permission error:", err);
      heard.permission("orientation", "failed");
      return false;
    },
  );
}

function orient_stop(): void {
  if (orient_event !== null) globalThis.removeEventListener(orient_event, on_orient, true);
  orient_event = null;
}

export const geo = { has: geo_has(), start: geo_start, stop: geo_stop };
export const orient = { has: orient_has(), gated: orient_gated(), silence: orient_silence(), permit: orient_permit, start: orient_start, stop: orient_stop };
