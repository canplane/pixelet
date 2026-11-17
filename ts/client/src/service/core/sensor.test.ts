// @pxl/client/service/core/sensor.test.ts

import { beforeEach, expect, test, vi } from "vitest";

import * as platform from "@/service/platform/sensor.ts";
import {
  SENSOR_LISTENER,
  setPlatform,
  startOrientation,
  worldClosed,
  worldOpen,
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
  ORIENTATION: {
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
  setPlatform();
  vi.clearAllMocks();
});

// Each sensor starts only while a world is open, from whichever trigger: a
// grant that lands with no world open, or a permit answered after the world
// closed, starts nothing.
test("a sensor starts only while a world is open", async () => {
  SENSOR_LISTENER.permission("geolocation", "granted");
  SENSOR_LISTENER.permission("orientation", "granted");
  expect(await startOrientation()).toBe(false);
  await answered();
  expect(platform.GEOLOCATION.start).not.toHaveBeenCalled();
  expect(platform.ORIENTATION.start).not.toHaveBeenCalled();

  worldOpen(1);
  worldClosed(1);
  await answered();
  expect(platform.ORIENTATION.start).not.toHaveBeenCalled();

  vi.clearAllMocks();
  worldOpen(1);
  await answered();
  expect(platform.GEOLOCATION.start).toHaveBeenCalledTimes(1);
  expect(platform.ORIENTATION.start).toHaveBeenCalledTimes(1);
  worldClosed(1);
});

// A heading the platform does not permit is not started, and the press is
// told so.
test("a heading not permitted starts nothing", async () => {
  vi.mocked(platform.ORIENTATION.permit).mockResolvedValueOnce(false);
  worldOpen(1);
  await answered();
  expect(platform.ORIENTATION.start).not.toHaveBeenCalled();
  expect(await startOrientation()).toBe(true);
  expect(platform.ORIENTATION.start).toHaveBeenCalledTimes(1);
  worldClosed(1);
});
