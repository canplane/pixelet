// @pxl/client/service/sys/fs/storage.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { ERRNO_EBADMSG } from "@xpute/kit/status/errno.spec.ts";
import { FaultError } from "@xpute/kit/status/error.ts";

import { path } from "@/abi/file.spec.ts";
import type { RefusedFile } from "@/abi/overlay.ts";
import { follow_refused_files, set_aside_by_path } from "./refusal.ts";
import {
  is_storage_key,
  commit_storage,
  open_storage,
  set_aside,
  type Storage,
} from "./storage.ts";

const SETTINGS = path.STORAGE.SETTINGS.ID;

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

test("set_aside - a refused file is copied, read back, then its slot emptied", () => {
  const s = storage({
    "pxl:settings": "AAAA",
    "pxl:settings:aside:7": "older",
  });
  assert.deepStrictEqual(set_aside(s, SETTINGS, 7), true);
  assert.deepStrictEqual(s.map.get("pxl:settings"), "");
  assert.deepStrictEqual(s.map.get("pxl:settings:aside:7"), "older");
  assert.deepStrictEqual(s.map.get("pxl:settings:aside:7-1"), "AAAA");
});

test("set_aside - storage that refuses the copy keeps the slot as it was", () => {
  const s = storage({ "pxl:settings": "AAAA" }, true);
  assert.deepStrictEqual(set_aside(s, SETTINGS, 7), false);
  assert.deepStrictEqual(s.map.get("pxl:settings"), "AAAA");
  assert.deepStrictEqual(set_aside(storage(), 99, 7), false);
});

test("open_storage - a file's bytes read back as committed, kept as text", () => {
  const s = storage();
  commit_storage(SETTINGS, new Uint8Array([1, 0, 255]), s);
  assert.deepStrictEqual(s.map.get("pxl:settings"), "AQD_");
  assert.deepStrictEqual([...open_storage(SETTINGS, s)], [1, 0, 255]);
});

test("open_storage - text that keeps no bytes is no file of bytes", () => {
  const s = storage({ "pxl:settings": '{"ao":"screen"}' });
  let errnum: number | undefined;
  try {
    open_storage(SETTINGS, s);
  } catch (err) {
    errnum = err instanceof FaultError ? err.errnum : undefined;
  }
  assert.deepStrictEqual(errnum, ERRNO_EBADMSG);
});

test("follow_refused_files - each refused file is set aside and emptied once", async () => {
  const s = storage({ "pxl:settings": '{"ao":"screen"}' });
  const told: string[] = [];
  const settings: RefusedFile = {
    path: [
      { tag: "int", val: path.STORAGE.ID },
      { tag: "int", val: SETTINGS },
    ],
  };
  const tell = (f: RefusedFile): void => void told.push(f.path.map((t) => t.val).join("/"));
  follow_refused_files([settings], tell, set_aside_by_path(s));
  follow_refused_files([settings], tell, set_aside_by_path(s));
  await Promise.resolve();
  assert.deepStrictEqual(told, [`${path.STORAGE.ID}/${SETTINGS}`]);
  assert.deepStrictEqual(s.map.get("pxl:settings"), "");
  follow_refused_files([], tell, set_aside_by_path(s));
  assert.deepStrictEqual(told.length, 1);
});

test("storage - the keys kept are the core's files and those set aside, no other", () => {
  assert.deepStrictEqual(
    ["pxl:settings", "pxl:settings:aside:7-1", "pxl:lang", "pxl:ui"].map(
      is_storage_key,
    ),
    [true, true, false, false],
  );
});
