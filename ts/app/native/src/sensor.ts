// @pxl/native/sensor.ts

/**
 * Position and heading through expo-horizon-location, the expo-location fork
 * that reads position without Google Play Services (Horizon OS has none).
 * Each crosses as the page's do, the heading where Safari's
 * `webkitCompassHeading` goes.
 */

import { AppState } from "react-native";
import * as Location from "expo-horizon-location";

import type { SensorHeard } from "@pxl/client/bracket.tsx";
import { PLATFORM } from "./module.ts";

/** Set once at boot, before any sensor starts. */
let heard: SensorHeard;

export function hear(given: SensorHeard): void {
  heard = given;
}

function geo_has(): boolean {
  return true;
}

function orient_has(): boolean {
  return PLATFORM.headingAvailable();
}

function orient_gated(): boolean {
  return false;
}

function orient_silence(): boolean {
  return false;
}

function orient_permit(): Promise<boolean> {
  return Promise.resolve(true);
}

let granted: boolean | null = null;

/** Only reads an answered permission: asking opens an Android activity,
 * and the resume restarted the sensors, which asked again in a loop. */
async function permitted(): Promise<boolean> {
  const now = await Location.getForegroundPermissionsAsync();
  const { status } = now.status === Location.PermissionStatus.UNDETERMINED ? await Location.requestForegroundPermissionsAsync() : now;
  granted = status === Location.PermissionStatus.GRANTED;
  return granted;
}

/** A grant made in the device's settings reaches the kernel on return. */
export function follow(): () => void {
  const back = AppState.addEventListener("change", (state) => {
    if (state === "active") recheck();
  });
  return () => back.remove();
}

/** CoreLocation says "no course" and "no accuracy" as a negative, the page's API as null. */
function known(v: number | null): number | null {
  return v !== null && v >= 0 ? v : null;
}

function recheck(): void {
  if (granted !== false) return;
  void Location.getForegroundPermissionsAsync().then(({ status }) => {
    if (granted !== false || status !== Location.PermissionStatus.GRANTED) return;
    granted = true;
    heard.permission("geolocation", "granted");
    heard.permission("orientation", "granted");
  });
}

interface Watch {
  subscription: Location.LocationSubscription | null;
  stopped: boolean;
}

function watch_end(watch: Watch): void {
  watch.stopped = true;
  watch.subscription?.remove();
  watch.subscription = null;
}

/** A subscription that arrives after its stop is let go at once. */
function watch_keep(watch: Watch, subscription: Location.LocationSubscription): void {
  if (watch.stopped) subscription.remove();
  else watch.subscription = subscription;
}

let geolocation: Watch | null = null;

function geo_fail(watch: Watch, error: "denied" | "other"): void {
  if (geolocation !== watch) return;
  geo_stop();
  heard.positionError(error);
}

function geo_start(): void {
  if (geolocation !== null) return;
  const watch: Watch = { subscription: null, stopped: false };
  geolocation = watch;
  void (async () => {
    if (!(await permitted())) return geo_fail(watch, "denied");
    const subscription = await Location.watchPositionAsync(
      { accuracy: Location.Accuracy.BestForNavigation, timeInterval: 1000, distanceInterval: 0 },
      (pos) => {
        const { latitude, longitude, altitude, altitudeAccuracy, accuracy, heading } = pos.coords;
        // An altitude with no accuracy is none measured.
        heard.position(longitude, latitude, known(altitudeAccuracy) === null ? null : altitude, known(accuracy), known(heading));
      },
      // Android's watch never ends on its own (expo's watch callbacks leave
      // onLocationError empty, and the fused provider keeps the request);
      // iOS rides out locationUnknown as CoreLocation keeps trying, and ends
      // the stream on any other failure. An ended watch is said as one: the
      // kernel starts it again on a grant or a return to the foreground.
      () => geo_fail(watch, "other"),
    );
    watch_keep(watch, subscription);
  })().catch(() => geo_fail(watch, "other"));
}

function geo_stop(): void {
  if (geolocation === null) return;
  watch_end(geolocation);
  geolocation = null;
}

let orientation: Watch | null = null;

function orient_fail(watch: Watch, refused: boolean): void {
  if (orientation !== watch) return;
  orient_stop();
  if (refused) heard.permission("orientation", "denied");
  else heard.orientationLost();
}

function orient_start(): void {
  if (orientation !== null) return;
  const watch: Watch = { subscription: null, stopped: false };
  orientation = watch;
  void (async () => {
    if (!(await permitted())) return orient_fail(watch, true);
    const subscription = await Location.watchHeadingAsync(
      (h) => {
        const heading = h.trueHeading >= 0 ? h.trueHeading : h.magHeading;
        heard.orientation(heading, null, null, null);
      },
      (reason) => {
        console.error("[sensor] heading:", reason);
        orient_fail(watch, false);
      },
    );
    watch_keep(watch, subscription);
  })().catch((err) => {
    console.error("[sensor] heading:", err);
    orient_fail(watch, false);
  });
}

function orient_stop(): void {
  if (orientation === null) return;
  watch_end(orientation);
  orientation = null;
}

export const geo = { has: geo_has(), start: geo_start, stop: geo_stop };
export const orient = { has: orient_has(), gated: orient_gated(), silence: orient_silence(), permit: orient_permit, start: orient_start, stop: orient_stop };
