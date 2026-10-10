// @pxl/native/platform/sensor.ts

/**
 * Position through expo-horizon-location, the expo-location fork that reads
 * it without Google Play Services (Horizon OS has none). It crosses as the
 * page's does.
 */

import { AppState } from "react-native";
import * as Location from "expo-horizon-location";

import { GeolocationErrorCode, PermissionAnswer, Sensor } from "@/kernel/abi/cmd.spec.ts";
import { kernel_permission, kernel_position, kernel_position_error } from "@/kernel/control/sensor.ts";

export function geolocationSupported(): boolean {
  return true;
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

/** CoreLocation says "no course" and "no accuracy" as a negative, the page's API as null. */
function known(v: number | null): number | null {
  return v !== null && v >= 0 ? v : null;
}

function recheck(): void {
  if (granted !== false) return;
  void Location.getForegroundPermissionsAsync().then(({ status }) => {
    if (granted !== false || status !== Location.PermissionStatus.GRANTED) return;
    granted = true;
    kernel_permission({ sensor: Sensor.Geolocation, answer: PermissionAnswer.Granted });
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
        kernel_position({ lon: longitude, lat: latitude, altitude, accuracy: known(accuracy), course: known(heading) });
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
