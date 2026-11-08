// @pxl/client/service/sensor.ts

/**
 * The device's sensors as a browser grants them to a page. iOS gates the
 * compass behind a permission prompt, which must be raised inside a gesture.
 */

import { geo_fix, listener } from "./sensor-listener.ts";
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

let geo_watch: number | null = null;

/** How long the world waits on a location prompt that no one answers before
 * it opens at the core's fallback place. An answer that comes later still
 * moves it. A device needs no such wait, because its prompt is the system's
 * and holds the app itself. The value is a product choice. */
const PROMPT_MS = 5000;

function geo_prompt_wait(): void {
  setTimeout(() => {
    navigator.permissions?.query({ name: "geolocation" }).then(
      (status) => {
        if (geo_watch !== null && status.state === "prompt") {
          listener.positionError("timeout");
        }
      },
      () => {},
    );
  }, PROMPT_MS);
}

function geo_stop(): void {
  if (geo_watch !== null) navigator.geolocation.clearWatch(geo_watch);
  geo_watch = null;
}

function geo_start(): void {
  if (geo_watch !== null || !("geolocation" in navigator)) return;
  geo_watch = navigator.geolocation.watchPosition(
    (pos) => geo_fix(pos.coords),
    (err) => {
      // A watch keeps trying through POSITION_UNAVAILABLE and TIMEOUT, and ends
      // only on PERMISSION_DENIED (https://www.w3.org/TR/geolocation/#watchposition-method).
      // So it is left running on any other error, and the core shows the
      // failure until a fix lands.
      switch (err.code) {
        case err.PERMISSION_DENIED:
          geo_stop();
          listener.positionError("denied");
          break;
        case err.POSITION_UNAVAILABLE:
          listener.positionError("unavailable");
          break;
        case err.TIMEOUT:
          listener.positionError("timeout");
          break;
        default:
          listener.positionError("other");
      }
    },
    // No timeout is set (PositionOptions.timeout is Infinity by default). With
    // one, TIMEOUT would fire on a clock while no fix comes, and each would
    // cost the core a turn for nothing.
    { enableHighAccuracy: true, maximumAge: 5000 },
  );
  geo_prompt_wait();
}

let orient_event: string | null = null;

function on_orient(e: Event): void {
  const o = e as IosOrientationEvent;
  listener.orientation(
    o.webkitCompassHeading ?? null,
    o.alpha,
    o.beta,
    o.gamma,
  );
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
  listener.permission("orientation", "asking");
  return prompt.then(
    (permission) => {
      const granted = permission === "granted";
      if (granted) orient_listen("deviceorientation");
      listener.permission("orientation", granted ? "granted" : "denied");
      return granted;
    },
    (err) => {
      console.error("[sensor] orientation permission error:", err);
      listener.permission("orientation", "failed");
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

export const geo: GeolocationSensor = {
  has: "geolocation" in navigator,
  start: geo_start,
  stop: geo_stop,
};
export const orient: OrientationSensor = {
  has: orient_has(),
  gated: orient_gated(),
  silence: true,
  permit: orient_permit,
  start: orient_start,
  stop: orient_stop,
};
