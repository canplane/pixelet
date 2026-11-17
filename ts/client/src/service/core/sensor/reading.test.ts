// @pxl/client/service/core/sensor/reading.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import {
  defaultDeviceLocationStore,
  defaultDeviceOrientationStore,
  SENSOR_STATUS_READY,
  setLocation,
  setOrientation,
} from "./reading.ts";

test("setOrientation - a heading changes past its dead zone", () => {
  // A heading changes only past 1.5° of the one held, measured on the circle,
  // and a heading that appears or goes always changes.
  const s = defaultDeviceOrientationStore();
  setOrientation(s, 359);
  assert.equal(s.status, SENSOR_STATUS_READY);
  assert.equal(s.orientation?.heading, 359);
  for (const [to, held] of [
    [0.4, 359],
    [0.6, 0.6],
    [359.2, 0.6],
    [359, 359],
    [null, null],
    [null, null],
    [10, 10],
  ] as const) {
    setOrientation(s, to);
    assert.equal(s.orientation?.heading ?? null, held, `to ${to}`);
  }
});

test("setOrientation - the first reading makes the sensor Ready, even one without a heading", () => {
  const s = defaultDeviceOrientationStore();
  setOrientation(s, null);
  assert.equal(s.status, SENSOR_STATUS_READY);
  assert.deepEqual(s.orientation, { heading: null });
});

test("setLocation - a fix moves past its dead zone", () => {
  // A fix moves the location only past about 1.1 m across, 0.00001°, or
  // 0.5 m in altitude.
  const s = defaultDeviceLocationStore();
  const at: [number, number] = [127, 37.5];
  setLocation(s, at, 10, 5, null);
  assert.equal(s.state.status, SENSOR_STATUS_READY);
  const near: [number, number] = [at[0] + 0.000009, at[1] - 0.000009];
  setLocation(s, near, 10.4, 5, null);
  assert.deepEqual([s.location?.coord, s.location?.alt], [at, 10]);
  for (const [coord, alt] of [
    [[at[0] + 0.000011, at[1]], 10],
    [[at[0] + 0.000011, at[1] - 0.000011], 10],
    [[at[0] + 0.000011, at[1] - 0.000011], 10.6],
    [[at[0] + 0.000011, at[1] - 0.000011], null],
  ] as [[number, number], number | null][]) {
    setLocation(s, coord, alt, 5, null);
    assert.deepEqual([s.location?.coord, s.location?.alt], [coord, alt]);
  }
});

test("setLocation - a fix past 100 m of accuracy is taken only when it improves", () => {
  const s = defaultDeviceLocationStore();
  const at: [number, number] = [127, 37.5];
  const far: [number, number] = [127.01, 37.51];
  setLocation(s, at, null, 50, null);
  setLocation(s, far, null, 150, null);
  assert.deepEqual([s.location?.coord, s.location?.accuracy], [at, 50]);

  const coarse = defaultDeviceLocationStore();
  setLocation(coarse, at, null, 500, null);
  setLocation(coarse, far, null, 300, null);
  assert.deepEqual(
    [coarse.location?.coord, coarse.location?.accuracy],
    [far, 300],
  );
});

test("setLocation - accuracy and course refresh without movement", () => {
  const s = defaultDeviceLocationStore();
  const at: [number, number] = [127, 37.5];
  setLocation(s, at, 10, 5, null);
  setLocation(s, [at[0] + 0.000005, at[1]], 10.2, 4, 90);
  assert.deepEqual(s.location, {
    coord: at,
    alt: 10,
    accuracy: 4,
    course: 90,
  });
});
