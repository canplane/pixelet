// @xpute/kit/wire/xtp/spec.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import {
  ScalarType,
  SequenceType,
  SpecialType,
  TYPE_IS_KNOWN,
  TYPE_IS_SCALAR,
  TYPE_IS_SEQ,
  TYPE_IS_SPECIAL,
} from "@xpute/kit/wire/xtp/spec.ts";

const vals = (group: object): [string, number][] => Object.entries(group);

test("each type is of its group's kind and one the wire carries", () => {
  for (const [name, type] of vals(ScalarType)) {
    assert.ok(
      TYPE_IS_SCALAR(type) && TYPE_IS_KNOWN(type),
      `ScalarType.${name}`,
    );
  }
  for (const [name, type] of vals(SequenceType)) {
    assert.ok(TYPE_IS_SEQ(type) && TYPE_IS_KNOWN(type), `SequenceType.${name}`);
  }
  for (const [name, type] of vals(SpecialType)) {
    assert.ok(TYPE_IS_SPECIAL(type), `SpecialType.${name}`);
  }
  // GRAFT is left out: the encoder splices a graft in, so it is never on the
  // wire as itself.
  assert.ok(
    TYPE_IS_KNOWN(SpecialType.NIL) && TYPE_IS_KNOWN(SpecialType.BRANCH),
  );
});

test("no two types share a byte", () => {
  const seen = new Map<number, string>();
  for (const [group, of] of [
    ["ScalarType", ScalarType],
    ["SequenceType", SequenceType],
    ["SpecialType", SpecialType],
  ] as const) {
    for (const [name, type] of vals(of)) {
      const was = seen.get(type);
      assert.equal(was, undefined, `${group}.${name} is ${was}'s byte`);
      seen.set(type, `${group}.${name}`);
    }
  }
});
