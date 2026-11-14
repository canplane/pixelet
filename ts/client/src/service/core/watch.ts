// @pxl/client/service/core/watch.ts

import { type Sensor, SENSOR_GEOLOCATION } from "@/abi/types.enum.ts";
import * as watch from "@/abi/watch.ts";
import type { Remote } from "@xpute/link";
import * as platform from "@/service/platform/sensor.ts";
import { geolocation_fix, MEASURE_PLACE } from "@/service/platform/sensor-listener.ts";

/** Serves `watch` on `remote`, the core's start and stop of the device's
 * sensors, before the core boots. */
export function serve_watch(remote: Remote): void {
  watch.mount(remote, { start: sensor_start, stop: sensor_stop });
}

function sensor_start(s: Sensor): void {
  // A build made for measuring has its place given and starts no sensor, so
  // that no permission prompt holds it on any platform.
  if (__PERF_DRIVE__) {
    if (s === SENSOR_GEOLOCATION) geolocation_fix(MEASURE_PLACE);
    return;
  }
  if (s === SENSOR_GEOLOCATION) {
    platform.GEOLOCATION.start();
  } else platform.ORIENT.start();
}

function sensor_stop(s: Sensor): void {
  if (s === SENSOR_GEOLOCATION) {
    platform.GEOLOCATION.stop();
  } else platform.ORIENT.stop();
}
