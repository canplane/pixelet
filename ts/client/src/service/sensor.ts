// @pxl/client/service/sensor.ts

/**
 * The device's sensors as the browser grants them. iOS gates the compass
 * behind a prompt that must be raised inside the gesture.
 */

import type {
  GeolocationSensor,
  OrientationSensor,
  SensorListener,
} from "./types.ts";

/** Set once at boot, before any sensor starts. */
let listener: SensorListener;

export function listen(given: SensorListener): void {
  listener = given;
}

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

/** How long the world waits on a location prompt no one answers, then opens at
 * the kernel's fallback; a later answer moves it. A device's prompt is the
 * system's, and holds the app itself. A product number. */
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
    (pos) => {
      const {
        latitude,
        longitude,
        altitude,
        altitudeAccuracy,
        accuracy,
        heading,
      } = pos.coords;
      // An altitude with no accuracy is none measured: a desktop's location gives 0.
      listener.position(
        longitude,
        latitude,
        altitudeAccuracy === null ? null : altitude,
        accuracy,
        heading,
      );
    },
    (err) => {
      // A watch keeps trying through POSITION_UNAVAILABLE and TIMEOUT and ends
      // only on PERMISSION_DENIED (https://www.w3.org/TR/geolocation/#watchposition-method),
      // so it is left running on any other; the kernel shows the failure until a fix lands.
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
    // No timeout: one would raise TIMEOUT on a clock while no fix comes
    // (PositionOptions.timeout, Infinity by default), each a turn for nothing.
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

/** A grant in the browser's site settings reaches the kernel, since nothing re-asks after a refusal. */
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

/** The prompt is called before any await, so it stays inside the gesture. */
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
