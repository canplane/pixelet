// @pxl/client/driver/namespace/storage.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/host/ipc/sys.ts";

import { root, storage as slots } from "@/abi/host.spec.ts";
import { follow_refused_files, set_aside_by_root } from "./refusal.ts";
import {
  commit_storage,
  open_storage,
  set_aside,
  type Storage,
} from "./storage.ts";

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
  assert.deepStrictEqual(set_aside(s, slots.SETTINGS, 7), true);
  assert.deepStrictEqual(s.map.get("pxl:settings"), "");
  assert.deepStrictEqual(s.map.get("pxl:settings:aside:7"), "older");
  assert.deepStrictEqual(s.map.get("pxl:settings:aside:7-1"), "AAAA");
});

test("set_aside - storage that refuses the copy keeps the slot as it was", () => {
  const s = storage({ "pxl:settings": "AAAA" }, true);
  assert.deepStrictEqual(set_aside(s, slots.SETTINGS, 7), false);
  assert.deepStrictEqual(s.map.get("pxl:settings"), "AAAA");
  assert.deepStrictEqual(set_aside(storage(), 99, 7), false);
});

test("open_storage - a file's bytes read back as committed, kept as text", () => {
  const s = storage();
  commit_storage(BigInt(slots.SETTINGS), new Uint8Array([1, 0, 255]), s);
  assert.deepStrictEqual(s.map.get("pxl:settings"), "AQD_");
  assert.deepStrictEqual(
    [...open_storage(BigInt(slots.SETTINGS), s)],
    [1, 0, 255],
  );
});

test("open_storage - text that keeps no bytes is no file of bytes", () => {
  const s = storage({ "pxl:settings": '{"ao":"screen"}' });
  let errnum: number | undefined;
  try {
    open_storage(BigInt(slots.SETTINGS), s);
  } catch (err) {
    errnum = err instanceof FileError ? err.errnum : undefined;
  }
  assert.deepStrictEqual(errnum, Errno.EBADMSG);
});

test("follow_refused_files - each refused file is set aside and emptied once", async () => {
  const s = storage({ "pxl:settings": '{"ao":"screen"}' });
  const told: string[] = [];
  const settings = { root: root.STORAGE, path: String(slots.SETTINGS) };
  const tell = (f: { root: number; path: string }): void =>
    void told.push(`${f.root}/${f.path}`);
  follow_refused_files([settings], tell, set_aside_by_root(s));
  follow_refused_files([settings], tell, set_aside_by_root(s));
  await Promise.resolve();
  assert.deepStrictEqual(told, [`${root.STORAGE}/${slots.SETTINGS}`]);
  assert.deepStrictEqual(s.map.get("pxl:settings"), "");
  follow_refused_files([], tell, set_aside_by_root(s));
  assert.deepStrictEqual(told.length, 1);
});
