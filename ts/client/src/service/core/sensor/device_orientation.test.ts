// @pxl/client/service/core/sensor/device_orientation.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import {
  ORIENTATION_COMMAND_START,
  ORIENTATION_COMMAND_STOP,
  on_orient_lost,
  orient_close,
  orient_event,
  orient_stop,
  orient_unavailable,
  platform,
  request_orient,
  take_orient_cmd,
} from "./device_orientation.ts";
import {
  device_orient_store,
  SENSOR_STATUS_LOADING,
  SENSOR_STATUS_READY,
  SENSOR_STATUS_UNAVAILABLE,
} from "./reading.ts";

/** A page's compass: no prompt, and silence is a loss. */
function page_compass(): void {
  Object.assign(platform(), {
    supported: true,
    needs_permission: false,
    silence_is_loss: true,
  });
}

test("orient_unavailable - a missing compass sets the compass's status alone", () => {
  // It stops the listener and reads unavailable from then on, the platform's
  // report kept.
  page_compass();
  orient_unavailable();
  assert.ok(platform().supported, "the report kept");
  assert.equal(take_orient_cmd(), ORIENTATION_COMMAND_STOP);
  assert.ok(!request_orient());
  const store = device_orient_store();
  assert.deepEqual(
    [store.status, store.is_tracking_orient],
    [SENSOR_STATUS_UNAVAILABLE, false],
  );
});

test("orient_stop - a hidden spell stops the heading, and asking again starts it", () => {
  // Hiding stops a tracked heading and keeps its status; asking again starts
  // it afresh, a failed compass alike.
  page_compass();
  orient_close();
  take_orient_cmd();
  assert.ok(request_orient());
  assert.equal(take_orient_cmd(), ORIENTATION_COMMAND_START);
  orient_event(10, null);
  orient_stop();
  orient_stop();
  assert.equal(take_orient_cmd(), ORIENTATION_COMMAND_STOP);
  assert.equal(device_orient_store().orient, null);
  assert.equal(device_orient_store().status, SENSOR_STATUS_READY);
  assert.ok(request_orient());
  assert.equal(take_orient_cmd(), ORIENTATION_COMMAND_START);
  assert.equal(device_orient_store().status, SENSOR_STATUS_LOADING);

  on_orient_lost();
  assert.ok(request_orient());
  assert.equal(take_orient_cmd(), ORIENTATION_COMMAND_START);
});
