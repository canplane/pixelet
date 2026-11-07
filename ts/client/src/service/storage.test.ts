// @pxl/client/service/storage.test.ts

import { afterEach, expect, test, vi } from "vitest";

import { keys } from "./storage.ts";

afterEach(() => vi.unstubAllGlobals());

test("storage - a page refused its storage keeps no keys, and does not throw", () => {
  vi.stubGlobal("localStorage", undefined);
  expect(keys()).toEqual([]);
  Object.defineProperty(globalThis, "localStorage", {
    configurable: true,
    get() {
      throw new DOMException("denied", "SecurityError");
    },
  });
  expect(keys()).toEqual([]);
});

test("storage - the keys are those kept", () => {
  vi.stubGlobal("localStorage", { a: "1", b: "2" });
  expect(keys()).toEqual(["a", "b"]);
});
