// @pxl/client/service/core/sensor.test.ts

import { beforeEach, expect, test, vi } from "vitest";

import * as platform from "@/service/platform/sensor.ts";
import {
  SENSOR_LISTENER,
  set_platform,
  start_orient,
  world_closed,
  world_open,
} from "./sensor.ts";

vi.mock("@/abi/sensor.ts", () => ({
  pos: vi.fn(async () => {}),
  heading: vi.fn(async () => {}),
  location: vi.fn(async () => {}),
}));
vi.mock("@/service/platform/sensor.ts", () => ({
  follow: vi.fn(() => () => {}),
  GEOLOCATION: {
    has: true,
    permit: vi.fn(async () => true),
    start: vi.fn(),
    stop: vi.fn(),
  },
  ORIENT: {
    has: true,
    silence: false,
    permit: vi.fn(async () => true),
    start: vi.fn(),
    stop: vi.fn(),
  },
}));

/** Lets the platform's permit answer. */
const answered = () => new Promise((r) => setTimeout(r, 0));

// The build's constant, folded into a bundle, is a global here.
vi.stubGlobal("__PERF_DRIVE__", false);

beforeEach(() => {
  set_platform();
  vi.clearAllMocks();
});

// Each sensor starts only while a world is open, from whichever trigger: a
// grant that lands with no world open, or a permit answered after the world
// closed, starts nothing.
test("a sensor starts only while a world is open", async () => {
  SENSOR_LISTENER.permission("geolocation", "granted");
  SENSOR_LISTENER.permission("orient", "granted");
  expect(await start_orient()).toBe(false);
  await answered();
  expect(platform.GEOLOCATION.start).not.toHaveBeenCalled();
  expect(platform.ORIENT.start).not.toHaveBeenCalled();

  world_open(1);
  world_closed(1);
  await answered();
  expect(platform.ORIENT.start).not.toHaveBeenCalled();

  vi.clearAllMocks();
  world_open(1);
  await answered();
  expect(platform.GEOLOCATION.start).toHaveBeenCalledTimes(1);
  expect(platform.ORIENT.start).toHaveBeenCalledTimes(1);
  world_closed(1);
});

// A heading the platform does not permit is not started, and the press is
// told so.
test("a heading not permitted starts nothing", async () => {
  vi.mocked(platform.ORIENT.permit).mockResolvedValueOnce(false);
  world_open(1);
  await answered();
  expect(platform.ORIENT.start).not.toHaveBeenCalled();
  expect(await start_orient()).toBe(true);
  expect(platform.ORIENT.start).toHaveBeenCalledTimes(1);
  world_closed(1);
});
