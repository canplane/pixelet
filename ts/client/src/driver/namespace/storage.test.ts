// @pxl/client/driver/namespace/storage.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/host/ipc/sys.ts";

import { root, storage as stored } from "@/abi/host.spec.ts";
import { follow_refused, set_aside_by_root } from "./refused.ts";
import {
  commit_storage,
  migrate,
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

test("migrate - once: a slot that holds anything is not written", () => {
  const s = storage({
    "pxl:settings": "AQ",
    "pxl:kept:settings": "AQIB",
  });
  migrate(s);
  assert.deepStrictEqual(s.map.get("pxl:kept:settings"), "AQIB");
});

test("migrate - the settings carry over as they are, for the kernel to read or refuse", () => {
  const s = storage({ "pxl:settings": '{"ao":"screen"}' });
  migrate(s);
  assert.deepStrictEqual(s.map.get("pxl:kept:settings"), '{"ao":"screen"}');
  const none = storage();
  migrate(none);
  assert.deepStrictEqual(none.map.size, 0);
});

test("migrate - the objects kept whole before their shards are not carried", () => {
  const s = storage({ "pxl:kept:objects": "DwAA", "pxl:object:v15": "AAAA" });
  migrate(s);
  assert.deepStrictEqual(s.map.size, 2);
});

test("set_aside - a refused file is copied, read back, then its slot emptied", () => {
  const s = storage({
    "pxl:kept:settings": "AAAA",
    "pxl:kept:settings:aside:7": "older",
  });
  assert.deepStrictEqual(set_aside(s, stored.SETTINGS, 7), true);
  assert.deepStrictEqual(s.map.get("pxl:kept:settings"), "");
  assert.deepStrictEqual(s.map.get("pxl:kept:settings:aside:7"), "older");
  assert.deepStrictEqual(s.map.get("pxl:kept:settings:aside:7-1"), "AAAA");
});

test("set_aside - storage that refuses the copy keeps the slot as it was", () => {
  const s = storage({ "pxl:kept:settings": "AAAA" }, true);
  assert.deepStrictEqual(set_aside(s, stored.SETTINGS, 7), false);
  assert.deepStrictEqual(s.map.get("pxl:kept:settings"), "AAAA");
  assert.deepStrictEqual(set_aside(storage(), 99, 7), false);
});

test("open_storage - a file's bytes read back as committed, kept as text", () => {
  const s = storage();
  commit_storage(BigInt(stored.SETTINGS), new Uint8Array([1, 0, 255]), s);
  assert.deepStrictEqual(s.map.get("pxl:kept:settings"), "AQD_");
  assert.deepStrictEqual(
    [...open_storage(BigInt(stored.SETTINGS), s)],
    [1, 0, 255],
  );
});

test("open_storage - text that keeps no bytes is no file of bytes", () => {
  const s = storage({ "pxl:kept:settings": '{"ao":"screen"}' });
  let errnum: number | undefined;
  try {
    open_storage(BigInt(stored.SETTINGS), s);
  } catch (err) {
    errnum = err instanceof FileError ? err.errnum : undefined;
  }
  assert.deepStrictEqual(errnum, Errno.EBADMSG);
});

test("follow_refused - each refused file is set aside and emptied once", () => {
  const s = storage({ "pxl:kept:settings": '{"ao":"screen"}' });
  const told: string[] = [];
  const settings = { root: root.STORAGE, path: String(stored.SETTINGS) };
  const tell = (f: { root: number; path: string }): void =>
    void told.push(`${f.root}/${f.path}`);
  follow_refused([settings], tell, set_aside_by_root(s));
  follow_refused([settings], tell, set_aside_by_root(s));
  assert.deepStrictEqual(told, [`${root.STORAGE}/${stored.SETTINGS}`]);
  assert.deepStrictEqual(s.map.get("pxl:kept:settings"), "");
  follow_refused([], tell, set_aside_by_root(s));
  assert.deepStrictEqual(told.length, 1);
});
