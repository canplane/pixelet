// @pxl/client/service/sys/fs/tile.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import {
  TILE_SPHERE,
  tileObject,
  type BASE_LAYERS,
} from "@/service/origin/publication.spec.ts";
import { decl } from "./source.ts";
import { version_of } from "./tile.ts";
import FIXTURE from "./tile.fixture.json" with { type: "json" };

const DECL = decl(JSON.stringify(FIXTURE.source))!;

test("version_of - each kind's tile at each zoom, a shard's own version or not, is the object the core made", () => {
  for (const { kind, object } of FIXTURE.tiles) {
    const k = kind as (typeof BASE_LAYERS)[number];
    const name = object.split("/")[1]!;
    assert.deepStrictEqual(
      tileObject(version_of(DECL, k, name), name),
      object,
      `${kind} ${name}`,
    );
  }
  assert.deepStrictEqual(
    tileObject(version_of(DECL, "earth", TILE_SPHERE), TILE_SPHERE),
    FIXTURE.sphere,
  );
});

test("decl - one this client cannot read is not handed over", () => {
  const whole = FIXTURE.source;
  const { version } = whole;
  assert.deepStrictEqual(
    decl(JSON.stringify(whole)) !== null,
    true,
    "the fixture's own is read",
  );
  const { shard: _, ...unsharded } = whole;
  for (const bad of [
    { ...whole, version: { ...version, terrain: "a/b" } },
    {
      ...whole,
      version: {
        terrain: version.terrain,
        place: version.place,
        division: version.division,
      },
    },
    { ...whole, shard: { "0103300": { version: { place: "" } } } },
    { ...whole, shard: [] as unknown },
    unsharded,
    { ...whole, min_zoom: 2 ** 32 + 4 },
    { ...whole, min_zoom: 4.5 },
    { ...whole, format: { ...whole.format, terrain: -1 } },
  ])
    assert.deepStrictEqual(
      decl(JSON.stringify(bad)),
      null,
      JSON.stringify(bad),
    );
  assert.deepStrictEqual(decl("not json"), null);
});
