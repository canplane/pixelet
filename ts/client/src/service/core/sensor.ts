// @pxl/client/service/core/sensor.ts

/** The device's sensors, the GPS and the compass, as a driver: the platform's
 * pair hands it what the device reports (`SENSOR_LISTENER`), and it keeps
 * every decision about them; their readings are in `sensor/reading.ts`. It
 * tells the core the three facts of them the core reads, each as it changes:
 * the device's position, the heading and the location's status
 * (`tell_core`). The sensors are watched only while a surface draws the
 * world, from its draw until the last such surface closes (`worlds`), and
 * stopped while the app is hidden. The location is asked for again when the
 * location card is pressed (`locate`), and the heading when follow is pressed
 * (`ask_heading`). */

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
import * as sensor_listener from "@/service/platform/sensor-listener.ts";
import { ui } from "@/service/core/core.ts";
import {
  type OrientationCommand,
  ORIENTATION_COMMAND_START,
  ORIENTATION_COMMAND_STOP,
  on_orient_lost,
  orient_close,
  orient_event,
  orient_permission,
  orient_stop,
  orient_unavailable,
  orient_watchdog_step,
  platform as orient_platform,
  request_orient,
  take_orient_cmd,
} from "./sensor/device_orientation.ts";
import {
  type GeolocationCommand,
  GEOLOCATION_COMMAND_START,
  GEOLOCATION_COMMAND_STOP,
  geolocation_close,
  geolocation_err,
  geolocation_fix,
  on_geolocation_permission_granted,
  platform as geolocation_platform,
  start_watching,
  stop_watching,
  take_geolocation_cmd,
} from "./sensor/geolocation.ts";
import {
  type LocationSensorStatus,
  device_location_store,
  device_orient_store,
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

function on_earth(lon: f64, lat: f64): boolean {
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
  pos: (lon, lat, alt_in, accuracy_in, course_in) => {
    const [alt, accuracy, course] = [
      finite(alt_in),
      finite(accuracy_in),
      finite(course_in),
    ];
    if (!(
      on_earth(lon, lat) &&
      (accuracy === null || accuracy >= 0) &&
      bearing(course)
    ))
      return;
    // A fix that comes without an accuracy is taken as having an infinite
    // error.
    geolocation_fix(lon, lat, alt, accuracy ?? Infinity, course);
    post();
  },
  pos_err: (err, watching) => {
    geolocation_err(err, watching);
    post();
  },
  orient: (compass_in, alpha_in) => {
    const [compass, alpha] = [finite(compass_in), finite(alpha_in)];
    if (!(bearing(compass) && bearing(alpha))) return;
    orient_event(compass, alpha);
    post();
  },
  orient_unavailable: () => {
    orient_unavailable();
    post();
  },
  // The compass gives no cause for its failure.
  orient_err: () => {
    on_orient_lost();
    post();
  },
  permission: (which, answer) => {
    if (which === "geolocation") {
      if (answer === "granted") on_geolocation_permission_granted();
    } else orient_permission(answer);
    post();
  },
};

/** Takes what the place's sensors are, once the client starts: a position, a
 * heading, whether the heading asks a permission inside a gesture, and
 * whether its silence is a loss. */
export function set_platform(): void {
  geolocation_platform().supported = platform.GEOLOCATION.has;
  const orient = orient_platform();
  orient.supported = platform.ORIENT.has;
  orient.needs_permission = platform.ORIENT.gated;
  orient.silence_is_loss = platform.ORIENT.silence;
}

/** The surfaces that draw the world. While one is open the sensors are
 * watched and the location's permission followed (`unfollow`). */
const worlds = new Set<u32>();
let unfollow: (() => void) | null = null;

/** The surface `surface` draws the world: the location starts where the
 * platform has one, and the heading is asked for where the platform gives
 * one. */
export function world_open(surface: u32): void {
  if (worlds.size === 0) unfollow = platform.follow();
  worlds.add(surface);
  start_watching();
  request_orient();
  post();
}

/** A world's surface closed. A surface can close after the one replacing it
 * has opened, so the sensors stop with the last world's surface. */
export function world_closed(surface: u32): void {
  worlds.delete(surface);
  if (worlds.size !== 0) return;
  unfollow?.();
  unfollow = null;
  geolocation_close();
  orient_close();
  post();
}

/** The app is hidden or shown again. Hiding stops every sensor that runs,
 * keeping what it read; the return starts every sensor the shell wants while
 * a world is open, one that stopped or failed alike. A heading behind a
 * permission still waits for a press. */
export function sensors_visibility(visible: boolean): void {
  if (!visible) {
    stop_watching();
    orient_stop();
  } else if (worlds.size !== 0) {
    start_watching();
    request_orient();
  }
  post();
}

/** Starts the location once it is permitted. It must run inside a press,
 * which raises the permission's prompt or its settings. */
export async function locate(): Promise<void> {
  if (!(await platform.GEOLOCATION.permit())) return;
  start_watching();
  post();
}

/** Asks for the heading, as follow is pressed. */
export function ask_heading(): void {
  request_orient();
  post();
}

/** A build made for measuring has its place given and starts no sensor, so
 * that no permission prompt holds it on any platform. Its fix comes as the
 * platform's do, in an entry of its own. This is the measuring build's
 * fixture (the performance skill's `PXL_PERF_DRIVE`), the one branch on the
 * build here, and no platform's case. */
const GEOLOCATION_CMD: Readonly<Record<GeolocationCommand, () => void>> = {
  [GEOLOCATION_COMMAND_START]: () => {
    if (!__PERF_DRIVE__) return platform.GEOLOCATION.start();
    queueMicrotask(() =>
      sensor_listener.geolocation_fix(sensor_listener.MEASURE_PLACE),
    );
  },
  [GEOLOCATION_COMMAND_STOP]: () => platform.GEOLOCATION.stop(),
};

const ORIENTATION_CMD: Readonly<Record<OrientationCommand, () => void>> = {
  [ORIENTATION_COMMAND_START]: () => {
    if (!__PERF_DRIVE__) platform.ORIENT.start();
  },
  [ORIENTATION_COMMAND_STOP]: () => platform.ORIENT.stop(),
};

/** The watchdog's one wake, armed for the earliest time it asks. */
let wake: { at: number; timer: ReturnType<typeof setTimeout> } | null = null;

function wake_at(at: number): void {
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
  const at = orient_watchdog_step();
  if (at !== null) wake_at(at);
  const g = take_geolocation_cmd();
  if (g !== null) GEOLOCATION_CMD[g]();
  const o = take_orient_cmd();
  if (o !== null) ORIENTATION_CMD[o]();
  tell_core();
  listeners.notify();
}

/** Watches the sensors' state (`sensor/reading.ts`). */
export const watch_sensor = listeners.watch;

/** The compass heading, 0-360 degrees, or null while there is none. */
function heading(): f64 | null {
  return device_orient_store().orient?.heading ?? null;
}

/** The device's position, its altitude over the ellipsoid where the device
 * gives one; null without a fix. */
function pos(): Position | null {
  const l = device_location_store().location;
  return l === null
    ? null
    : { lon: l.lon_lat[0], lat: l.lon_lat[1], alt: l.alt };
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
function location_status(): LocationStatus {
  return LOCATION_STATUS[device_location_store().state.status];
}

function is_same_pos(a: Position | null, b: Position | null): boolean {
  if (a === null || b === null) return a === b;
  return a.lon === b.lon && a.lat === b.lat && a.alt === b.alt;
}

/** The facts last told the core, none before the first. */
let told: {
  at: Position | null;
  heading: f64 | null;
  status: LocationStatus;
} | null = null;

/** Tells the core each fact that differs from what it was told last. */
function tell_core(): void {
  const [at, deg, status] = [pos(), heading(), location_status()];
  if (told === null || !is_same_pos(told.at, at)) void core.pos(ui, at);
  if (told === null || told.heading !== deg) void core.heading(ui, deg);
  if (told === null || told.status !== status) void core.location(ui, status);
  told = { at, heading: deg, status };
}
