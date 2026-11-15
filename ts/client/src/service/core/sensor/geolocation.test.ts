// @pxl/client/service/core/sensor/geolocation.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import {
  GEOLOCATION_COMMAND_START,
  GEOLOCATION_COMMAND_STOP,
  geolocation_close,
  geolocation_err,
  geolocation_fix,
  platform,
  start_watching,
  stop_watching,
  take_geolocation_cmd,
} from "./geolocation.ts";
import {
  device_location_store,
  SENSOR_STATUS_ERROR,
  SENSOR_STATUS_IDLE,
  SENSOR_STATUS_LOADING,
} from "./reading.ts";

/** Opens the location afresh on a platform that has one. */
function opened(): void {
  platform().supported = true;
  geolocation_close();
  take_geolocation_cmd();
  start_watching();
  assert.equal(take_geolocation_cmd(), GEOLOCATION_COMMAND_START);
}

test("geolocation_err - the platform says whether the watch runs on", () => {
  // A failure the watch rides out leaves it running, so asking again starts
  // nothing; one that ended it leaves nothing running and no location, so
  // asking again starts a watch.
  opened();
  geolocation_fix(126.9784, 37.5667, null, 5, null);
  geolocation_err("unavailable", true);
  let store = device_location_store();
  assert.ok(store.is_tracking_location);
  assert.notEqual(store.location, null);
  start_watching();
  assert.equal(take_geolocation_cmd(), null);

  geolocation_err("unavailable", false);
  store = device_location_store();
  assert.ok(!store.is_tracking_location);
  assert.equal(store.location, null);
  assert.equal(store.state.status, SENSOR_STATUS_ERROR);
  start_watching();
  assert.equal(take_geolocation_cmd(), GEOLOCATION_COMMAND_START);
  assert.equal(device_location_store().state.status, SENSOR_STATUS_LOADING);
});

test("stop_watching - a hidden spell keeps the location", () => {
  // Hiding the app stops the watch and keeps what it read: on the return the
  // location is still known, and reads as Loading until a fix of the new
  // watch.
  opened();
  geolocation_fix(126.9784, 37.5667, null, 5, null);
  stop_watching();
  assert.equal(take_geolocation_cmd(), GEOLOCATION_COMMAND_STOP);
  assert.notEqual(device_location_store().location, null);
  start_watching();
  assert.equal(take_geolocation_cmd(), GEOLOCATION_COMMAND_START);
  let store = device_location_store();
  assert.notEqual(store.location, null);
  assert.equal(store.state.status, SENSOR_STATUS_LOADING);

  geolocation_close();
  store = device_location_store();
  assert.equal(store.location, null);
  assert.equal(store.state.status, SENSOR_STATUS_IDLE);
});
