// @pxl/client/service/core/sensor/geolocation.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import {
  GEOLOCATION_COMMAND_START,
  GEOLOCATION_COMMAND_STOP,
  geolocationClose,
  geolocationError,
  geolocationFix,
  platform,
  startWatching,
  stopWatching,
  takeGeolocationCommand,
} from "./geolocation.ts";
import {
  deviceLocationStore,
  SENSOR_STATUS_ERROR,
  SENSOR_STATUS_IDLE,
  SENSOR_STATUS_LOADING,
} from "./reading.ts";

/** Opens the location afresh on a platform that has one. */
function opened(): void {
  platform().supported = true;
  geolocationClose();
  takeGeolocationCommand();
  startWatching();
  assert.equal(takeGeolocationCommand(), GEOLOCATION_COMMAND_START);
}

test("geolocationError - the platform says whether the watch runs on", () => {
  // A failure the watch rides out leaves it running, so asking again starts
  // nothing; one that ended it leaves nothing running and no location, so
  // asking again starts a watch.
  opened();
  geolocationFix(126.9784, 37.5667, null, 5, null);
  geolocationError("unavailable", true);
  let store = deviceLocationStore();
  assert.ok(store.isTrackingLocation);
  assert.notEqual(store.location, null);
  startWatching();
  assert.equal(takeGeolocationCommand(), null);

  geolocationError("unavailable", false);
  store = deviceLocationStore();
  assert.ok(!store.isTrackingLocation);
  assert.equal(store.location, null);
  assert.equal(store.state.status, SENSOR_STATUS_ERROR);
  startWatching();
  assert.equal(takeGeolocationCommand(), GEOLOCATION_COMMAND_START);
  assert.equal(deviceLocationStore().state.status, SENSOR_STATUS_LOADING);
});

test("stopWatching - a hidden spell keeps the location", () => {
  // Hiding the app stops the watch and keeps what it read: on the return the
  // location is still known, and reads as Loading until a fix of the new
  // watch.
  opened();
  geolocationFix(126.9784, 37.5667, null, 5, null);
  stopWatching();
  assert.equal(takeGeolocationCommand(), GEOLOCATION_COMMAND_STOP);
  assert.notEqual(deviceLocationStore().location, null);
  startWatching();
  assert.equal(takeGeolocationCommand(), GEOLOCATION_COMMAND_START);
  let store = deviceLocationStore();
  assert.notEqual(store.location, null);
  assert.equal(store.state.status, SENSOR_STATUS_LOADING);

  geolocationClose();
  store = deviceLocationStore();
  assert.equal(store.location, null);
  assert.equal(store.state.status, SENSOR_STATUS_IDLE);
});
