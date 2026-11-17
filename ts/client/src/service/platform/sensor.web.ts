// @pxl/client/service/platform/sensor.web.ts

/** iOS gates the compass behind a permission prompt, which must be raised
 * inside a gesture. */

import { coordinatesFix, geolocationFix, listener } from "./sensor-listener.ts";
import type { GeolocationSensor, OrientationSensor } from "./types.ts";

interface IosOrientationEvent extends DeviceOrientationEvent {
  webkitCompassHeading?: number;
}
type IosOrientationEventConstructor = typeof DeviceOrientationEvent & {
  requestPermission?: () => Promise<"granted" | "denied">;
};

function hasOrientation(): boolean {
  return typeof DeviceOrientationEvent !== "undefined";
}

function orientationGated(): boolean {
  return (
    hasOrientation() &&
    typeof (DeviceOrientationEvent as IosOrientationEventConstructor)
      .requestPermission === "function"
  );
}

let geolocationWatch: number | null = null;

function geolocationStop(): void {
  if (geolocationWatch !== null)
    navigator.geolocation.clearWatch(geolocationWatch);
  geolocationWatch = null;
}

function geolocationStart(): void {
  if (geolocationWatch !== null || !("geolocation" in navigator)) return;
  geolocationWatch = navigator.geolocation.watchPosition(
    (pos) => geolocationFix(coordinatesFix(pos.coords)),
    (err) => {
      // A watch keeps trying through POSITION_UNAVAILABLE and TIMEOUT, and ends
      // only on PERMISSION_DENIED (https://www.w3.org/TR/geolocation/#watchposition-method).
      // So it is left running on any other error, and the shell shows the
      // failure until a fix lands.
      switch (err.code) {
        case err.PERMISSION_DENIED:
          geolocationStop();
          listener.positionError("denied", false);
          break;
        case err.POSITION_UNAVAILABLE:
          listener.positionError("unavailable", true);
          break;
        case err.TIMEOUT:
          listener.positionError("timeout", true);
          break;
        default:
          listener.positionError("other", true);
      }
    },
    // No timeout is set (PositionOptions.timeout is Infinity by default). With
    // one, TIMEOUT would fire on a clock while no fix comes, and each would
    // cost the shell's sensors an entry for nothing.
    { enableHighAccuracy: true, maximumAge: 5000 },
  );
}

let orientationEvent: string | null = null;

/** An event with every field null is how a browser says it has no sensor; any
 * other is a reading. Its heading is iOS's compass heading, else the absolute
 * event's alpha, which turns counter-clockwise from north (`orientationStart`); one
 * with neither is a reading with no heading. */
function onOrientation(e: Event): void {
  const o = e as IosOrientationEvent;
  const compass = o.webkitCompassHeading ?? null;
  if (
    compass === null &&
    o.alpha === null &&
    o.beta === null &&
    o.gamma === null
  ) {
    listener.orientationUnavailable();
    return;
  }
  listener.orientation(
    compass ?? (o.alpha === null ? null : (360 - o.alpha) % 360),
  );
}

function orientationStart(): void {
  if (orientationEvent !== null) return;
  orientationEvent =
    "ondeviceorientationabsolute" in globalThis
      ? "deviceorientationabsolute"
      : "deviceorientation";
  globalThis.addEventListener(orientationEvent, onOrientation, true);
}

/** Follows the location permission, so that a grant made later in the
 * browser's site settings reaches the shell's sensors. Nothing asks again
 * after a refusal. */
export function follow(): () => void {
  let status: PermissionStatus | null = null;
  let stopped = false;
  const onChange = () =>
    listener.permission(
      "geolocation",
      status?.state === "granted" ? "granted" : "denied",
    );
  navigator.permissions
    ?.query({ name: "geolocation" })
    .then((s) => {
      if (stopped) return;
      status = s;
      s.addEventListener("change", onChange);
    })
    .catch(() => {});
  return () => {
    stopped = true;
    status?.removeEventListener("change", onChange);
  };
}

/** Asks before any await, so that inside a press a prompt stays inside the
 * gesture. A decision already made answers without one, and with none made
 * outside a press the browser refuses to prompt (NotAllowedError), which is no
 * answer yet: a press asks. */
function orientationPermit(): Promise<boolean> {
  if (!orientationGated() || orientationEvent !== null)
    return Promise.resolve(true);
  const asked = (DeviceOrientationEvent as IosOrientationEventConstructor)
    .requestPermission!();
  return asked.then(
    (permission) => {
      const granted = permission === "granted";
      if (!granted) listener.permission("orientation", "denied");
      return granted;
    },
    (err) => {
      if (err instanceof DOMException && err.name === "NotAllowedError") {
        return false;
      }
      console.error("[sensor] orientation permission error:", err);
      listener.permission("orientation", "failed");
      return false;
    },
  );
}

function orientationStop(): void {
  if (orientationEvent !== null) {
    globalThis.removeEventListener(orientationEvent, onOrientation, true);
  }
  orientationEvent = null;
}

export const GEOLOCATION: GeolocationSensor = {
  has: "geolocation" in navigator,
  // The watch raises the browser's prompt itself, wherever the browser still
  // asks. A page cannot open the browser's settings.
  permit: () => Promise.resolve(true),
  start: geolocationStart,
  stop: geolocationStop,
};
export const ORIENTATION: OrientationSensor = {
  has: hasOrientation(),
  silence: true,
  permit: orientationPermit,
  start: orientationStart,
  stop: orientationStop,
};
