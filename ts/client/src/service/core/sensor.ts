// @pxl/client/service/core/sensor.ts

/** The device's sensors, the GPS and the compass, as a driver: the platform's
 * pair hands it what the device reports (`SENSOR_LISTENER`), and it keeps
 * every decision about them; their readings are in `sensor/reading.ts`. It
 * tells the core the three facts of them the core reads, each as it changes:
 * the device's position, the heading and the location's status
 * (`tellCore`). The sensors are watched only while a surface draws the
 * world, from its draw until the last such surface closes (`worlds`), and
 * stopped while the app is hidden. Each sensor has one start, which holds
 * that gate and which every trigger calls: a world's draw, the return from
 * hidden, a permission granted, and a press, the location card's (`locate`)
 * or the location button's (`startOrientation`). */

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { ListenerSet } from "@xpute/kit/lang/listener.ts";
import * as core from "@/abi/sensor.ts";
import type { Position } from "@/abi/sensor.ts";
import {
  type LocationStatus,
  LOCATION_STATUS_ERROR,
  LOCATION_STATUS_IDLE,
  LOCATION_STATUS_LOADING,
  LOCATION_STATUS_READY,
} from "@/abi/sensor.enum.ts";
import type { SensorListener } from "@/service/platform/types.ts";
import * as platform from "@/service/platform/sensor.ts";
import * as sensorListener from "@/service/platform/sensor-listener.ts";
import { ui } from "@/service/core/core.ts";
import {
  type OrientationCommand,
  ORIENTATION_COMMAND_START,
  ORIENTATION_COMMAND_STOP,
  onOrientationLost,
  orientationGranted,
  orientationClose,
  orientationEvent,
  orientationStop,
  orientationUnavailable,
  orientationWatchdogStep,
  platform as orientationPlatform,
  requestOrientation,
  takeOrientationCommand,
} from "./sensor/device_orientation.ts";
import {
  type GeolocationCommand,
  GEOLOCATION_COMMAND_START,
  GEOLOCATION_COMMAND_STOP,
  geolocationClose,
  geolocationError,
  geolocationFix,
  geolocationGranted,
  platform as geolocationPlatform,
  startWatching,
  stopWatching,
  takeGeolocationCommand,
} from "./sensor/geolocation.ts";
import {
  type LocationSensorStatus,
  deviceLocationStore,
  deviceOrientationStore,
  SENSOR_STATUS_ERROR,
  SENSOR_STATUS_IDLE,
  SENSOR_STATUS_LOADING,
  SENSOR_STATUS_READY,
} from "./sensor/reading.ts";

/** A reading the device could not give, such as NaN or an infinity, is taken
 * as none. */
function finite(n: number | null): number | null {
  return n !== null && Number.isFinite(n) ? n : null;
}

function onEarth(lon: f64, lat: f64): boolean {
  return lon >= -180 && lon <= 180 && lat >= -90 && lat <= 90;
}

/** Whether `deg` is absent or a bearing in degrees, as a compass or a course
 * gives it. */
function bearing(deg: f64 | null): boolean {
  return deg === null || (deg >= 0 && deg <= 360);
}

/** What the platform's pair reports, each an entry of the driver. A reading
 * out of its range is dropped whole. */
export const SENSOR_LISTENER: SensorListener = {
  pos: (lon, lat, altitudeIn, accuracyIn, courseIn) => {
    const [altEllipsoid, accuracy, course] = [
      finite(altitudeIn),
      finite(accuracyIn),
      finite(courseIn),
    ];
    if (!(
      onEarth(lon, lat) &&
      (accuracy === null || accuracy >= 0) &&
      bearing(course)
    ))
      return;
    // A fix that comes without an accuracy is taken as having an infinite
    // error.
    geolocationFix(lon, lat, altEllipsoid, accuracy ?? Infinity, course);
    post();
  },
  positionError: (error, watching) => {
    geolocationError(error, watching);
    post();
  },
  orientation: (headingIn) => {
    const heading = finite(headingIn);
    if (!bearing(heading)) return;
    orientationEvent(heading);
    post();
  },
  orientationUnavailable: () => {
    orientationUnavailable();
    post();
  },
  // The compass gives no cause for its failure.
  orientationError: () => {
    onOrientationLost();
    post();
  },
  // A grant clears the sensor's failure and starts it; a refused compass
  // reads as failed.
  permission: (which, answer) => {
    if (answer === "granted") {
      if (which === "geolocation") {
        geolocationGranted();
        startLocation();
      } else {
        orientationGranted();
        void startOrientation();
      }
    } else if (which === "orientation") onOrientationLost();
    post();
  },
};

/** Takes what the place's sensors are, once the client starts: a position, a
 * heading, and whether the heading's silence is a loss. */
export function setPlatform(): void {
  geolocationPlatform().supported = platform.GEOLOCATION.has;
  const orientation = orientationPlatform();
  orientation.supported = platform.ORIENTATION.has;
  orientation.silenceIsLoss = platform.ORIENTATION.silence;
}

/** The surfaces that draw the world. While one is open the sensors are
 * watched and the location's permission followed (`unfollow`). */
const worlds = new Set<u32>();
let unfollow: (() => void) | null = null;

/** The surface `surface` draws the world, and the sensors start. */
export function worldOpen(surface: u32): void {
  if (worlds.size === 0) unfollow = platform.follow();
  worlds.add(surface);
  startLocation();
  void startOrientation();
  post();
}

/** A world's surface closed. A surface can close after the one replacing it
 * has opened, so the sensors stop with the last world's surface. */
export function worldClosed(surface: u32): void {
  worlds.delete(surface);
  if (worlds.size !== 0) return;
  unfollow?.();
  unfollow = null;
  geolocationClose();
  orientationClose();
  post();
}

/** The app is hidden or shown again. Hiding stops every sensor that runs,
 * keeping what it read; the return starts each sensor again, one that
 * stopped or failed alike, a heading already permitted without a press. */
export function sensorsVisibility(visible: boolean): void {
  if (!visible) {
    stopWatching();
    orientationStop();
  } else {
    startLocation();
    void startOrientation();
  }
  post();
}

/** Starts the location while a world is open. The watch asks the permission
 * itself where it is not yet decided. */
function startLocation(): void {
  if (worlds.size !== 0) startWatching();
}

/** Starts the heading while a world is open, once the platform permits it,
 * and resolves whether it does: never where the platform has no heading. The platform answers from the decision it
 * holds, and asks only inside a press: the location button's calls this from the
 * press itself. A world closed while it answers starts nothing. */
export async function startOrientation(): Promise<boolean> {
  if (worlds.size === 0 || !platform.ORIENTATION.has) return false;
  const permitted = await platform.ORIENTATION.permit();
  if (permitted && worlds.size !== 0) {
    requestOrientation();
    post();
  }
  return permitted;
}

/** Starts the location once it is permitted. It must run inside a press,
 * which raises the permission's prompt or its settings. */
export async function locate(): Promise<void> {
  if (!(await platform.GEOLOCATION.permit())) return;
  startLocation();
  post();
}

/** A build made for measuring has its place given and starts no sensor, so
 * that no permission prompt holds it on any platform. Its fix comes as the
 * platform's do, in an entry of its own. This is the measuring build's
 * fixture (the measure skill's `PXL_PERF_DRIVE`), the one branch on the
 * build here, and no platform's case. */
const GEOLOCATION_CMD: Readonly<Record<GeolocationCommand, () => void>> = {
  [GEOLOCATION_COMMAND_START]: () => {
    if (!__PERF_DRIVE__) return platform.GEOLOCATION.start();
    queueMicrotask(() =>
      sensorListener.geolocationFix(sensorListener.MEASURE_PLACE),
    );
  },
  [GEOLOCATION_COMMAND_STOP]: () => platform.GEOLOCATION.stop(),
};

const ORIENTATION_CMD: Readonly<Record<OrientationCommand, () => void>> = {
  [ORIENTATION_COMMAND_START]: () => {
    if (!__PERF_DRIVE__) platform.ORIENTATION.start();
  },
  [ORIENTATION_COMMAND_STOP]: () => platform.ORIENTATION.stop(),
};

/** The watchdog's one wake, armed for the earliest time it asks. */
let wake: { at: number; timer: ReturnType<typeof setTimeout> } | null = null;

function wakeAt(at: number): void {
  if (wake !== null && wake.at <= at) return;
  if (wake !== null) clearTimeout(wake.timer);
  wake = {
    at,
    timer: setTimeout(() => {
      wake = null;
      post();
    }, at - performance.now()),
  };
}

const listeners = new ListenerSet();

/** Ends each entry of the driver: the compass's watchdog judges, the
 * platform follows the last commands, the core is told the facts that
 * changed, and what watches the sensors is told to look again. */
function post(): void {
  const at = orientationWatchdogStep();
  if (at !== null) wakeAt(at);
  const g = takeGeolocationCommand();
  if (g !== null) GEOLOCATION_CMD[g]();
  const o = takeOrientationCommand();
  if (o !== null) ORIENTATION_CMD[o]();
  tellCore();
  listeners.notify();
}

/** Watches the sensors' state (`sensor/reading.ts`). */
export const watchSensor = listeners.watch;

/** The compass heading, 0-360 degrees, or null while there is none. */
function heading(): f64 | null {
  return deviceOrientationStore().orientation?.heading ?? null;
}

/** The device's position, its altitude over the ellipsoid where the device
 * gives one; null without a fix. */
function pos(): Position | null {
  const l = deviceLocationStore().location;
  return l === null
    ? null
    : { lon: l.coord[0], lat: l.coord[1], alt_ellipsoid: l.altEllipsoid };
}

/** The location's status as the core takes it. */
const LOCATION_STATUS: Readonly<Record<LocationSensorStatus, LocationStatus>> =
  {
    [SENSOR_STATUS_IDLE]: LOCATION_STATUS_IDLE,
    [SENSOR_STATUS_LOADING]: LOCATION_STATUS_LOADING,
    [SENSOR_STATUS_READY]: LOCATION_STATUS_READY,
    [SENSOR_STATUS_ERROR]: LOCATION_STATUS_ERROR,
  };

/** The location's status; ready once a fix came. */
function locationStatus(): LocationStatus {
  return LOCATION_STATUS[deviceLocationStore().state.status];
}

function isSamePosition(a: Position | null, b: Position | null): boolean {
  if (a === null || b === null) return a === b;
  return (
    a.lon === b.lon && a.lat === b.lat && a.alt_ellipsoid === b.alt_ellipsoid
  );
}

/** The facts last told the core, none before the first. */
let told: {
  at: Position | null;
  heading: f64 | null;
  status: LocationStatus;
} | null = null;

/** Tells the core each fact that differs from what it was told last. */
function tellCore(): void {
  const [at, deg, status] = [pos(), heading(), locationStatus()];
  if (told === null || !isSamePosition(told.at, at)) void core.pos(ui, at);
  if (told === null || told.heading !== deg) void core.heading(ui, deg);
  if (told === null || told.status !== status) void core.location(ui, status);
  told = { at, heading: deg, status };
}
