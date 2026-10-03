// @pxl/native/platform/sensor.ts

/**
 * Position and heading through expo-horizon-location, the expo-location fork
 * that reads position without Google Play Services (Horizon OS has none).
 * Each crosses as the page's do, the heading where Safari's
 * `webkitCompassHeading` goes.
 */

import { AppState } from "react-native";
import * as Location from "expo-horizon-location";

import { GeolocationErrorCode, PermissionAnswer, Sensor } from "@/kernel/abi/cmd.spec.ts";
import { kernel_orientation, kernel_orientation_lost, kernel_permission, kernel_position, kernel_position_error } from "@/kernel/control/sensor.ts";
import { PLATFORM } from "./module.ts";

export function geolocationSupported(): boolean {
  return true;
}

export function orientationSupported(): boolean {
  return PLATFORM.headingAvailable();
}

export function orientationNeedsPermission(): boolean {
  return false;
}

export function orientationSilenceIsLoss(): boolean {
  return false;
}

export function orientationPermit(): Promise<boolean> {
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
export function permissionFollow(): () => void {
  const back = AppState.addEventListener("change", (state) => {
    if (state === "active") recheck();
  });
  return () => back.remove();
}

function recheck(): void {
  if (granted !== false) return;
  void Location.getForegroundPermissionsAsync().then(({ status }) => {
    if (granted !== false || status !== Location.PermissionStatus.GRANTED) return;
    granted = true;
    kernel_permission({ sensor: Sensor.Geolocation, answer: PermissionAnswer.Granted });
    kernel_permission({ sensor: Sensor.Orientation, answer: PermissionAnswer.Granted });
  });
}

interface Watch {
  subscription: Location.LocationSubscription | null;
  stopped: boolean;
}

function watchEnd(watch: Watch): void {
  watch.stopped = true;
  watch.subscription?.remove();
  watch.subscription = null;
}

/** A subscription that arrives after its stop is let go at once. */
function watchKeep(watch: Watch, subscription: Location.LocationSubscription): void {
  if (watch.stopped) subscription.remove();
  else watch.subscription = subscription;
}

let geolocation: Watch | null = null;

function geolocationFail(watch: Watch, code: GeolocationErrorCode): void {
  if (geolocation !== watch) return;
  geolocationStop();
  kernel_position_error({ code });
}

export function geolocationStart(): void {
  if (geolocation !== null) return;
  const watch: Watch = { subscription: null, stopped: false };
  geolocation = watch;
  void (async () => {
    if (!(await permitted())) return geolocationFail(watch, GeolocationErrorCode.PermissionDenied);
    const subscription = await Location.watchPositionAsync(
      { accuracy: Location.Accuracy.BestForNavigation, timeInterval: 1000, distanceInterval: 0 },
      (pos) => {
        const { latitude, longitude, altitude, accuracy, heading } = pos.coords;
        kernel_position({ lon: longitude, lat: latitude, altitude, accuracy: accuracy ?? Infinity, course: heading });
      },
      () => geolocationFail(watch, GeolocationErrorCode.PositionUnavailable),
    );
    watchKeep(watch, subscription);
  })().catch(() => geolocationFail(watch, GeolocationErrorCode.Other));
}

export function geolocationStop(): void {
  if (geolocation === null) return;
  watchEnd(geolocation);
  geolocation = null;
}

let orientation: Watch | null = null;

function orientationFail(watch: Watch, refused: boolean): void {
  if (orientation !== watch) return;
  orientationRemove();
  if (refused) kernel_permission({ sensor: Sensor.Orientation, answer: PermissionAnswer.Denied });
  else kernel_orientation_lost();
}

export function orientationListen(): void {
  if (orientation !== null) return;
  const watch: Watch = { subscription: null, stopped: false };
  orientation = watch;
  void (async () => {
    if (!(await permitted())) return orientationFail(watch, true);
    const subscription = await Location.watchHeadingAsync(
      (h) => {
        const heading = h.trueHeading >= 0 ? h.trueHeading : h.magHeading;
        kernel_orientation({ webkit_compass_heading: heading, alpha: null, beta: null, gamma: null });
      },
      (reason) => {
        console.error("[sensor] heading:", reason);
        orientationFail(watch, false);
      },
    );
    watchKeep(watch, subscription);
  })().catch((err) => {
    console.error("[sensor] heading:", err);
    orientationFail(watch, false);
  });
}

export function orientationRemove(): void {
  if (orientation === null) return;
  watchEnd(orientation);
  orientation = null;
}
