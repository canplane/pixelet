// @pxl/client/service/platform/sensor.native.ts

/**
 * Position and heading through expo-horizon-location, the fork of
 * expo-location that reads position without Google Play Services, which
 * Horizon OS lacks. Both reach the listener in the same shape as the page's,
 * with the heading in the place of Safari's `webkitCompassHeading`.
 */

import { AppState } from "react-native";
import * as Location from "expo-horizon-location";

import { coords_fix, geo_fix, listener } from "./sensor-listener.ts";
import type { GeolocationSensor, OrientationSensor } from "./types.ts";
import { PLATFORM } from "./pxl-platform.ts";

let granted: boolean | null = null;

/** Asks for the permission only while it is undetermined, and otherwise only
 * reads the answer. Asking opens an Android activity, and the app's resume
 * from it restarts the sensors, so asking every time would loop. */
async function geo_permit(): Promise<boolean> {
  const now = await Location.getForegroundPermissionsAsync();
  const { status } =
    now.status === Location.PermissionStatus.UNDETERMINED
      ? await Location.requestForegroundPermissionsAsync()
      : now;
  granted = status === Location.PermissionStatus.GRANTED;
  return granted;
}

/** Rechecks the permission whenever the app comes back to the foreground, so
 * that a grant made in the device's settings reaches the core. */
export function follow(): () => void {
  const back = AppState.addEventListener("change", (state) => {
    if (state === "active") recheck();
  });
  return () => back.remove();
}

function recheck(): void {
  if (granted !== false) return;
  void Location.getForegroundPermissionsAsync().then(({ status }) => {
    if (granted !== false || status !== Location.PermissionStatus.GRANTED) {
      return;
    }
    granted = true;
    listener.permission("geolocation", "granted");
    listener.permission("orientation", "granted");
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

/** `watchPositionAsync` answers later, so a subscription can arrive after the
 * watch was stopped. It is then let go at once. */
function watch_keep(
  watch: Watch,
  subscription: Location.LocationSubscription,
): void {
  if (watch.stopped) subscription.remove();
  else watch.subscription = subscription;
}

let geolocation: Watch | null = null;

function geo_fail(watch: Watch, error: "denied" | "other"): void {
  if (geolocation !== watch) return;
  geo_stop();
  listener.positionError(error);
}

function geo_start(): void {
  if (geolocation !== null) return;
  const watch: Watch = { subscription: null, stopped: false };
  geolocation = watch;
  void (async () => {
    if (!(await geo_permit())) return geo_fail(watch, "denied");
    const subscription = await Location.watchPositionAsync(
      {
        accuracy: Location.Accuracy.BestForNavigation,
        timeInterval: 1000,
        distanceInterval: 0,
      },
      (pos) => geo_fix(coords_fix(pos.coords)),
      // Android's watch never ends on its own: expo's watch callbacks leave
      // onLocationError empty, and the fused provider keeps the request. iOS
      // rides out locationUnknown while CoreLocation keeps trying, and ends
      // the stream on any other failure. An ended watch is reported as a
      // failure, and the core starts the watch again on a grant or on a
      // return to the foreground.
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
  if (refused) listener.permission("orientation", "denied");
  else listener.orientationLost();
}

function orient_start(): void {
  if (orientation !== null) return;
  const watch: Watch = { subscription: null, stopped: false };
  orientation = watch;
  void (async () => {
    if (!(await geo_permit())) return orient_fail(watch, true);
    const subscription = await Location.watchHeadingAsync(
      (h) => {
        const heading = h.trueHeading >= 0 ? h.trueHeading : h.magHeading;
        listener.orientation(heading, null, null, null);
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

export const geo: GeolocationSensor = {
  has: true,
  start: geo_start,
  stop: geo_stop,
};
export const orient: OrientationSensor = {
  has: PLATFORM.headingAvailable(),
  gated: false,
  silence: false,
  // The compass is granted with the location, which is asked for when the
  // compass's watch starts.
  permit: () => Promise.resolve(true),
  start: orient_start,
  stop: orient_stop,
};
