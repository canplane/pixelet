// @pxl/client/service/sys/fs/storage.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { ComponentError } from "@xpute/link";
import {
  ERROR_CODE_BAD_MESSAGE,
  ERROR_CODE_IO,
  ERROR_CODE_NO_ENTRY,
} from "@xpute/host/abi/fs/types.enum.ts";

import { path } from "@/abi/file.spec.ts";
import {
  isStorageKey,
  commitStorage,
  openStorage,
  setAside,
  setStorage,
  type Storage,
} from "./storage.ts";

const SETTINGS = path.STORAGE.SETTINGS.ID;

function code(f: () => unknown): unknown {
  try {
    f();
  } catch (err) {
    return err instanceof ComponentError ? err.payload : undefined;
  }
  return undefined;
}

function storage(
  entries: Record<string, string> = {},
  refuses = false,
): Storage & { map: Map<string, string> } {
  const map = new Map(Object.entries(entries));
  return {
    map,
    get: (k) => map.get(k) ?? null,
    set: (k, v) => void (refuses || map.set(k, v)),
  };
}

test("setAside - a refused file is copied, read back, then its slot emptied", () => {
  const s = storage({
    "pxl:settings": "AAAA",
    "pxl:settings:aside:7": "older",
  });
  setStorage(s);
  setAside(SETTINGS, 7);
  assert.deepStrictEqual(s.map.get("pxl:settings"), "");
  assert.deepStrictEqual(s.map.get("pxl:settings:aside:7"), "older");
  assert.deepStrictEqual(s.map.get("pxl:settings:aside:7-1"), "AAAA");
});

test("setAside - storage that refuses the copy keeps the slot as it was", () => {
  const s = storage({ "pxl:settings": "AAAA" }, true);
  setStorage(s);
  assert.deepStrictEqual(
    code(() => setAside(SETTINGS, 7)),
    ERROR_CODE_IO,
  );
  assert.deepStrictEqual(s.map.get("pxl:settings"), "AAAA");
  assert.deepStrictEqual(
    code(() => setAside(99, 7)),
    ERROR_CODE_NO_ENTRY,
  );
});

test("openStorage - a file's bytes read back as committed, kept as text", () => {
  const s = storage();
  setStorage(s);
  commitStorage(SETTINGS, new Uint8Array([1, 0, 255]));
  assert.deepStrictEqual(s.map.get("pxl:settings"), "AQD_");
  assert.deepStrictEqual([...openStorage(SETTINGS)], [1, 0, 255]);
});

test("openStorage - text that keeps no bytes is no file of bytes", () => {
  setStorage(storage({ "pxl:settings": '{"ao":"screen"}' }));
  assert.deepStrictEqual(
    code(() => openStorage(SETTINGS)),
    ERROR_CODE_BAD_MESSAGE,
  );
});

test("storage - the keys kept are the core's files and those set aside, no other", () => {
  assert.deepStrictEqual(
    ["pxl:settings", "pxl:settings:aside:7-1", "pxl:lang", "pxl:ui"].map(
      isStorageKey,
    ),
    [true, true, false, false],
  );
});
