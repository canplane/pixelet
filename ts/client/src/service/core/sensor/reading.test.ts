// @pxl/client/service/core/sensor/reading.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import {
  default_DeviceLocationStore,
  default_DeviceOrientationStore,
  SENSOR_STATUS_READY,
  set_location,
  set_orient,
} from "./reading.ts";

test("set_orient - a heading changes past its dead zone", () => {
  // A heading changes only past 1.5° of the one held, measured on the circle,
  // and a heading that appears or goes always changes.
  const s = default_DeviceOrientationStore();
  set_orient(s, 359);
  assert.equal(s.status, SENSOR_STATUS_READY);
  assert.equal(s.orient?.heading, 359);
  for (const [to, held] of [
    [0.4, 359],
    [0.6, 0.6],
    [359.2, 0.6],
    [359, 359],
    [null, null],
    [null, null],
    [10, 10],
  ] as const) {
    set_orient(s, to);
    assert.equal(s.orient?.heading ?? null, held, `to ${to}`);
  }
});

test("set_orient - the first reading makes the sensor Ready, even one without a heading", () => {
  const s = default_DeviceOrientationStore();
  set_orient(s, null);
  assert.equal(s.status, SENSOR_STATUS_READY);
  assert.deepEqual(s.orient, { heading: null });
});

test("set_location - a fix moves past its dead zone", () => {
  // A fix moves the location only past about 1.1 m across, 0.00001°, or
  // 0.5 m in altitude.
  const s = default_DeviceLocationStore();
  const at: [number, number] = [127, 37.5];
  set_location(s, at, 10, 5, null);
  assert.equal(s.state.status, SENSOR_STATUS_READY);
  const near: [number, number] = [at[0] + 0.000009, at[1] - 0.000009];
  set_location(s, near, 10.4, 5, null);
  assert.deepEqual([s.location?.lon_lat, s.location?.alt], [at, 10]);
  for (const [lon_lat, alt] of [
    [[at[0] + 0.000011, at[1]], 10],
    [[at[0] + 0.000011, at[1] - 0.000011], 10],
    [[at[0] + 0.000011, at[1] - 0.000011], 10.6],
    [[at[0] + 0.000011, at[1] - 0.000011], null],
  ] as [[number, number], number | null][]) {
    set_location(s, lon_lat, alt, 5, null);
    assert.deepEqual([s.location?.lon_lat, s.location?.alt], [lon_lat, alt]);
  }
});

test("set_location - a fix past 100 m of accuracy is taken only when it improves", () => {
  const s = default_DeviceLocationStore();
  const at: [number, number] = [127, 37.5];
  const far: [number, number] = [127.01, 37.51];
  set_location(s, at, null, 50, null);
  set_location(s, far, null, 150, null);
  assert.deepEqual([s.location?.lon_lat, s.location?.accuracy], [at, 50]);

  const coarse = default_DeviceLocationStore();
  set_location(coarse, at, null, 500, null);
  set_location(coarse, far, null, 300, null);
  assert.deepEqual(
    [coarse.location?.lon_lat, coarse.location?.accuracy],
    [far, 300],
  );
});

test("set_location - accuracy and course refresh without movement", () => {
  const s = default_DeviceLocationStore();
  const at: [number, number] = [127, 37.5];
  set_location(s, at, 10, 5, null);
  set_location(s, [at[0] + 0.000005, at[1]], 10.2, 4, 90);
  assert.deepEqual(s.location, {
    lon_lat: at,
    alt: 10,
    accuracy: 4,
    course: 90,
  });
});
