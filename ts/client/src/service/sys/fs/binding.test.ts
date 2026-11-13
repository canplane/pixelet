// @pxl/client/service/sys/fs/binding.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { FaultError } from "@xpute/kit/status/error.ts";

import { LEAF_ZOOM, path as tree } from "@/abi/file.spec.ts";
import { binding, object_key, type Path, walk } from "./binding.ts";
import { TOP_KEY } from "./object.ts";

const signal = new AbortController().signal;

function errnum(f: () => unknown): number | undefined {
  try {
    f();
  } catch (err) {
    return err instanceof FaultError ? err.errnum : undefined;
  }
  return undefined;
}

const T = tree.TERRAIN.ID;
const E = tree.EARTH.ID;
const S = tree.STORAGE.ID;
const O = tree.OBJECT.ID;

test("binding - a path that leads to no node, or whose data its node does not take, is no file, and reaches no backend", () => {
  const b = binding();
  const none: Path[] = [
    [],
    [T],
    [T, "0124"],
    [T, "%2e%2e"],
    [T, "0".repeat(LEAF_ZOOM + 1)],
    [T, 123],
    [T, "0123", "0"],
    [tree.PLACE.ID, tree.EARTH.SPHERE.ID],
    [E, tree.EARTH.SPHERE.ID, 0],
    [tree.ASSET.ID, "1"],
    [tree.ASSET.ID, 1, 1],
    [S],
    [S, String(0)],
    [S, tree.STORAGE.SETTINGS.ID, 0],
    [O, "0123"],
    [O, tree.OBJECT.EDIT.ID],
    [O, tree.OBJECT.TOP.ID, 0],
    ["0", "0123"],
    [999, 0],
  ];
  for (const p of none) {
    assert.deepStrictEqual(
      errnum(() => b.open(p, signal)),
      Errno.ENOENT,
      JSON.stringify(p),
    );
  }
});

test("binding - only the settings and the objects' edits are written", () => {
  const b = binding();
  const read_only: Path[] = [
    [T, "0123"],
    [tree.ASSET.ID, 1],
    [S, String(0)],
    [S, tree.STORAGE.SETTINGS.ID, 0],
    [O, tree.OBJECT.TOP.ID],
    [O, "00000000"],
    [O, tree.OBJECT.EDIT.ID, 0],
  ];
  for (const p of read_only) {
    assert.deepStrictEqual(
      errnum(() => b.commit!(p, new Uint8Array())),
      Errno.EROFS,
      JSON.stringify(p),
    );
  }
});

test("walk - each node takes one token and hands the rest to the child it names", () => {
  assert.deepStrictEqual(walk([E, tree.EARTH.SPHERE.ID])?.[1], []);
  assert.deepStrictEqual(walk([E, "0123"])?.[1], ["0123"]);
  assert.deepStrictEqual(walk([O, tree.OBJECT.TOP.ID, 7])?.[1], [7]);
  assert.deepStrictEqual(walk([999]), undefined);
});

test("object_key - the top and a shard, each by its own key", () => {
  assert.deepStrictEqual(object_key([O, tree.OBJECT.TOP.ID]), TOP_KEY);
  assert.deepStrictEqual(object_key([O, "00000000"]), "00000000");
});
