// @pxl/client/service/platform/sensor.native.ts

/**
 * Position and heading through expo-location. Both reach the listener in the
 * same shape as the page's, with the heading in the place of Safari's
 * `webkitCompassHeading`.
 */

import { AppState, Linking } from "react-native";
import * as Location from "expo-location";

import { coords_fix, geolocation_fix, listener } from "./sensor-listener.ts";
import type { GeolocationSensor, OrientationSensor } from "./types.ts";
import { PLATFORM } from "./pxl-platform.native.ts";

let granted: boolean | null = null;

/** The last look at the permission. iOS's requester keeps only the latest
 * request's promise, so a request made while another is out leaves the first
 * unanswered. Each look waits for the one before and reads the answer it
 * left, so one prompt answers every sensor that asked. */
let looking: Promise<unknown> = Promise.resolve();

function serial<T>(look: () => Promise<T>): Promise<T> {
  const next = looking.then(() => look());
  looking = next.catch(() => {});
  return next;
}

/** Asks for the permission only while it is undetermined, and otherwise only
 * reads the answer. Asking opens an Android activity, and the app's resume
 * from it restarts the sensors, so asking every time would loop. */
function geolocation_granted(): Promise<boolean> {
  return serial(async () => {
    const now = await Location.getForegroundPermissionsAsync();
    const { status } =
      now.status === Location.PermissionStatus.UNDETERMINED
        ? await Location.requestForegroundPermissionsAsync()
        : now;
    granted = status === Location.PermissionStatus.GRANTED;
    return granted;
  });
}

/** iOS never asks again after a refusal, and Android stops asking after the
 * second. The settings then grant it, and the shell starts the watch again on
 * the app's return to the foreground. A granted location still needs the
 * device's own location switched on. */
function geolocation_permit(): Promise<boolean> {
  return serial(async () => {
    const now = await Location.getForegroundPermissionsAsync();
    if (now.status !== Location.PermissionStatus.GRANTED) {
      if (!now.canAskAgain) {
        granted = false;
        await Linking.openSettings();
        return false;
      }
      const { status } = await Location.requestForegroundPermissionsAsync();
      granted = status === Location.PermissionStatus.GRANTED;
      if (!granted) return false;
    }
    return services_on();
  });
}

/** An Android phone turns the location on through Play Services' dialog,
 * which rejects when it is declined. iOS has no such dialog and resolves at
 * once, so its settings open, and the shell starts the watch again on the
 * app's return to the foreground. */
async function services_on(): Promise<boolean> {
  if (await Location.hasServicesEnabledAsync()) return true;
  try {
    await Location.enableNetworkProviderAsync();
  } catch {
    return false;
  }
  if (await Location.hasServicesEnabledAsync()) return true;
  await Linking.openSettings();
  return false;
}

/** Rechecks the permission whenever the app comes back to the foreground, so
 * that a grant made in the device's settings reaches the shell's sensors. */
export function follow(): () => void {
  const back = AppState.addEventListener("change", (state) => {
    if (state === "active") recheck();
  });
  return () => back.remove();
}

function recheck(): void {
  if (granted !== false) return;
  void serial(() => Location.getForegroundPermissionsAsync()).then(
    ({ status }) => {
      if (granted !== false || status !== Location.PermissionStatus.GRANTED) {
        return;
      }
      granted = true;
      listener.permission("geolocation", "granted");
      listener.permission("orient", "granted");
    },
  );
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

/** A device's watch ends on any failure. */
function geolocation_fail(
  watch: Watch,
  err: "denied" | "unavailable" | "other",
): void {
  if (geolocation !== watch) return;
  geolocation_stop();
  listener.pos_err(err, false);
}

function geolocation_start(): void {
  if (geolocation !== null) return;
  const watch: Watch = { subscription: null, stopped: false };
  geolocation = watch;
  void (async () => {
    if (!(await geolocation_granted()))
      return geolocation_fail(watch, "denied");
    // A watch the shell starts asks only for a permission not yet decided and
    // raises no settings dialog: the location switched off ends it, and the
    // press turns the location on (`services_on`).
    if (!(await Location.hasServicesEnabledAsync())) {
      return geolocation_fail(watch, "unavailable");
    }
    const subscription = await Location.watchPositionAsync(
      {
        accuracy: Location.Accuracy.BestForNavigation,
        timeInterval: 1000,
        distanceInterval: 0,
        mayShowUserSettingsDialog: false,
      },
      (pos) => geolocation_fix(coords_fix(pos.coords)),
      // Android's watch never ends on its own: expo's watch callbacks leave
      // onLocationError empty, and the fused provider keeps the request. iOS
      // rides out locationUnknown while CoreLocation keeps trying, and ends
      // the stream on any other failure. An ended watch is reported as a
      // failure, and the shell starts the watch again on a grant, a return to
      // the foreground or a press.
      () => geolocation_fail(watch, "other"),
    );
    watch_keep(watch, subscription);
  })().catch(() => geolocation_fail(watch, "other"));
}

function geolocation_stop(): void {
  if (geolocation === null) return;
  watch_end(geolocation);
  geolocation = null;
}

let orient: Watch | null = null;

function orient_fail(watch: Watch, refused: boolean): void {
  if (orient !== watch) return;
  orient_stop();
  if (refused) listener.permission("orient", "denied");
  else listener.orient_err();
}

function orient_start(): void {
  if (orient !== null) return;
  const watch: Watch = { subscription: null, stopped: false };
  orient = watch;
  void (async () => {
    if (!(await geolocation_granted())) return orient_fail(watch, true);
    const subscription = await Location.watchHeadingAsync(
      (h) => {
        const heading = h.trueHeading >= 0 ? h.trueHeading : h.magHeading;
        listener.orient(heading, null);
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
  if (orient === null) return;
  watch_end(orient);
  orient = null;
}

export const GEOLOCATION: GeolocationSensor = {
  has: true,
  permit: geolocation_permit,
  start: geolocation_start,
  stop: geolocation_stop,
};
export const ORIENT: OrientationSensor = {
  has: PLATFORM.has_heading(),
  gated: false,
  silence: false,
  // The compass is granted with the location, which is asked for when the
  // compass's watch starts.
  permit: () => Promise.resolve(true),
  start: orient_start,
  stop: orient_stop,
};
