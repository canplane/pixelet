// @pxl/client/service/platform/sensor.ts

/** iOS gates the compass behind a permission prompt, which must be raised
 * inside a gesture. */

import { coords_fix, geolocation_fix, listener } from "./sensor-listener.ts";
import type { GeolocationSensor, OrientationSensor } from "./types.ts";

interface IosOrientationEvent extends DeviceOrientationEvent {
  webkitCompassHeading?: number;
}
type IosOrientationEventCtor = typeof DeviceOrientationEvent & {
  requestPermission?: () => Promise<"granted" | "denied">;
};

function orient_has(): boolean {
  return typeof DeviceOrientationEvent !== "undefined";
}

function orient_gated(): boolean {
  return (
    orient_has() &&
    typeof (DeviceOrientationEvent as IosOrientationEventCtor)
      .requestPermission === "function"
  );
}

let geolocation_watch: number | null = null;

function geolocation_stop(): void {
  if (geolocation_watch !== null) navigator.geolocation.clearWatch(geolocation_watch);
  geolocation_watch = null;
}

function geolocation_start(): void {
  if (geolocation_watch !== null || !("geolocation" in navigator)) return;
  geolocation_watch = navigator.geolocation.watchPosition(
    (pos) => geolocation_fix(coords_fix(pos.coords)),
    (err) => {
      // A watch keeps trying through POSITION_UNAVAILABLE and TIMEOUT, and ends
      // only on PERMISSION_DENIED (https://www.w3.org/TR/geolocation/#watchposition-method).
      // So it is left running on any other error, and the core shows the
      // failure until a fix lands.
      switch (err.code) {
        case err.PERMISSION_DENIED:
          geolocation_stop();
          listener.pos_err("denied", false);
          break;
        case err.POSITION_UNAVAILABLE:
          listener.pos_err("unavailable", true);
          break;
        case err.TIMEOUT:
          listener.pos_err("timeout", true);
          break;
        default:
          listener.pos_err("other", true);
      }
    },
    // No timeout is set (PositionOptions.timeout is Infinity by default). With
    // one, TIMEOUT would fire on a clock while no fix comes, and each would
    // cost the core an entry for nothing.
    { enableHighAccuracy: true, maximumAge: 5000 },
  );
}

let orient_event: string | null = null;

/** An event with every field null is how a browser says it has no sensor; any
 * other is a reading, one with neither a compass heading nor alpha a reading
 * with no heading. */
function on_orient(e: Event): void {
  const o = e as IosOrientationEvent;
  const compass = o.webkitCompassHeading ?? null;
  if (
    compass === null &&
    o.alpha === null &&
    o.beta === null &&
    o.gamma === null
  ) {
    listener.orient_unavailable();
    return;
  }
  listener.orient(compass, o.alpha);
}

function orient_listen(name: string): void {
  if (orient_event !== null) return;
  orient_event = name;
  globalThis.addEventListener(name, on_orient, true);
}

function orient_start(): void {
  orient_listen(
    "ondeviceorientationabsolute" in globalThis
      ? "deviceorientationabsolute"
      : "deviceorientation",
  );
}

/** Follows the location permission, so that a grant made later in the
 * browser's site settings reaches the core. Nothing asks again after a
 * refusal. */
export function follow(): () => void {
  let status: PermissionStatus | null = null;
  let stopped = false;
  const on_change = () =>
    listener.permission(
      "geolocation",
      status?.state === "granted" ? "granted" : "denied",
    );
  navigator.permissions
    ?.query({ name: "geolocation" })
    .then((s) => {
      if (stopped) return;
      status = s;
      s.addEventListener("change", on_change);
    })
    .catch(() => {});
  return () => {
    stopped = true;
    status?.removeEventListener("change", on_change);
  };
}

/** Raises the prompt before any await, so that it stays inside the gesture. */
function orient_permit(): Promise<boolean> {
  if (!orient_gated() || orient_event !== null) return Promise.resolve(true);
  const prompt = (DeviceOrientationEvent as IosOrientationEventCtor)
    .requestPermission!();
  listener.permission("orient", "asking");
  return prompt.then(
    (permission) => {
      const granted = permission === "granted";
      if (granted) orient_listen("deviceorientation");
      listener.permission("orient", granted ? "granted" : "denied");
      return granted;
    },
    (err) => {
      console.error("[sensor] orientation permission error:", err);
      listener.permission("orient", "failed");
      return false;
    },
  );
}

function orient_stop(): void {
  if (orient_event !== null) {
    globalThis.removeEventListener(orient_event, on_orient, true);
  }
  orient_event = null;
}

export const GEOLOCATION: GeolocationSensor = {
  has: "geolocation" in navigator,
  // The watch raises the browser's prompt itself, wherever the browser still
  // asks. A page cannot open the browser's settings.
  permit: () => Promise.resolve(true),
  start: geolocation_start,
  stop: geolocation_stop,
};
export const ORIENT: OrientationSensor = {
  has: orient_has(),
  gated: orient_gated(),
  silence: true,
  permit: orient_permit,
  start: orient_start,
  stop: orient_stop,
};
