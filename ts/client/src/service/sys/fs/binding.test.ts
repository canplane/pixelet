// @pxl/client/service/sys/fs/binding.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { ComponentError } from "@xpute/link/link.ts";
import {
  ERROR_CODE_NO_ENTRY,
  ERROR_CODE_READ_ONLY,
} from "@xpute/host/abi/fs/types.enum.ts";

import { LEAF_ZOOM, path as tree } from "@/abi/file.spec.ts";
import { BINDING, type Path, walk } from "./binding.ts";

const SIGNAL = new AbortController().signal;

function code(f: () => unknown): unknown {
  try {
    f();
  } catch (err) {
    return err instanceof ComponentError ? err.payload : undefined;
  }
  return undefined;
}

const T = tree.TERRAIN.ID;
const I = tree.IMAGERY.ID;
const S = tree.STORAGE.ID;
const O = tree.OBJECT.ID;

test("BINDING - a path that leads to no node, or whose data its node does not take, is no file, and reaches no backend", () => {
  const none: Path[] = [
    [],
    [T],
    [T, "0124"],
    [T, "%2e%2e"],
    [T, "0".repeat(LEAF_ZOOM + 1)],
    [T, 123],
    [T, "0123", "0"],
    [tree.PLACE.ID, tree.IMAGERY.GLOBAL.ID],
    [I, tree.IMAGERY.GLOBAL.ID, 0],
    [tree.ASSET.ID, "1"],
    [tree.ASSET.ID, 1, 1],
    [S],
    [S, String(0)],
    [S, tree.STORAGE.SETTINGS.ID, 0],
    [O, "0123"],
    [O, tree.OBJECT.TOP.ID, 0],
    ["0", "0123"],
    [999, 0],
  ];
  for (const p of none) {
    assert.deepStrictEqual(
      code(() => BINDING.open(p, SIGNAL)),
      ERROR_CODE_NO_ENTRY,
      JSON.stringify(p),
    );
  }
});

test("BINDING - only the settings are written", () => {
  const readOnly: Path[] = [
    [T, "0123"],
    [tree.ASSET.ID, 1],
    [S, String(0)],
    [S, tree.STORAGE.SETTINGS.ID, 0],
    [O, tree.OBJECT.TOP.ID],
    [O, "00000000"],
  ];
  for (const p of readOnly) {
    assert.deepStrictEqual(
      code(() => BINDING.commit(p, new Uint8Array())),
      ERROR_CODE_READ_ONLY,
      JSON.stringify(p),
    );
  }
});

test("walk - each node takes one token and hands the rest to the child it names", () => {
  assert.deepStrictEqual(walk([I, tree.IMAGERY.GLOBAL.ID])?.[1], []);
  assert.deepStrictEqual(walk([I, "0123"])?.[1], ["0123"]);
  assert.deepStrictEqual(walk([O, tree.OBJECT.TOP.ID, 7])?.[1], [7]);
  assert.deepStrictEqual(walk([999]), undefined);
});
