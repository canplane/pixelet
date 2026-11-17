// @pxl/client/service/core/sensor/device_orientation.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import {
  ORIENTATION_COMMAND_START,
  ORIENTATION_COMMAND_STOP,
  onOrientationLost,
  orientationClose,
  orientationEvent,
  orientationStop,
  orientationUnavailable,
  platform,
  requestOrientation,
  takeOrientationCommand,
} from "./device_orientation.ts";
import {
  deviceOrientationStore,
  SENSOR_STATUS_LOADING,
  SENSOR_STATUS_READY,
  SENSOR_STATUS_UNAVAILABLE,
} from "./reading.ts";

/** The web compass: silence is a loss. */
function pageCompass(): void {
  Object.assign(platform(), { supported: true, silenceIsLoss: true });
}

test("orientationUnavailable - a missing compass sets the compass's status alone", () => {
  // It stops the listener and reads unavailable from then on, the platform's
  // report kept.
  pageCompass();
  orientationUnavailable();
  assert.ok(platform().supported, "the report kept");
  assert.equal(takeOrientationCommand(), ORIENTATION_COMMAND_STOP);
  assert.ok(!requestOrientation());
  const store = deviceOrientationStore();
  assert.deepEqual(
    [store.status, store.isTrackingOrientation],
    [SENSOR_STATUS_UNAVAILABLE, false],
  );
});

test("orientationStop - a hidden spell stops the heading, and asking again starts it", () => {
  // Hiding stops a tracked heading and keeps its status; asking again starts
  // it afresh, a failed compass alike.
  pageCompass();
  orientationClose();
  takeOrientationCommand();
  assert.ok(requestOrientation());
  assert.equal(takeOrientationCommand(), ORIENTATION_COMMAND_START);
  orientationEvent(10);
  orientationStop();
  orientationStop();
  assert.equal(takeOrientationCommand(), ORIENTATION_COMMAND_STOP);
  assert.equal(deviceOrientationStore().orientation, null);
  assert.equal(deviceOrientationStore().status, SENSOR_STATUS_READY);
  assert.ok(requestOrientation());
  assert.equal(takeOrientationCommand(), ORIENTATION_COMMAND_START);
  assert.equal(deviceOrientationStore().status, SENSOR_STATUS_LOADING);

  onOrientationLost();
  assert.ok(requestOrientation());
  assert.equal(takeOrientationCommand(), ORIENTATION_COMMAND_START);
});
