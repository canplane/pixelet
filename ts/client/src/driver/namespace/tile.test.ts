// @pxl/client/driver/namespace/tile.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { type BaseLayer, TILE_SPHERE, tileObject } from "./publication.spec.ts";
import { declaration } from "./source.ts";
import { version_of } from "./tile.ts";
import FIXTURE from "./tile.fixture.json" with { type: "json" };

const DECLARATION = declaration(JSON.stringify(FIXTURE.source))!;

test("version_of - each kind's tile at each zoom, a shard's own version or not, is the object the kernel made", () => {
  for (const { kind, object } of FIXTURE.tiles) {
    const k = kind as BaseLayer;
    const name = object.split("/")[1]!;
    assert.deepStrictEqual(
      tileObject(version_of(DECLARATION, k, name), name),
      object,
      `${kind} ${name}`,
    );
  }
  assert.deepStrictEqual(
    tileObject(version_of(DECLARATION, "earth", TILE_SPHERE), TILE_SPHERE),
    FIXTURE.sphere,
  );
});

test("declaration - one this client cannot read is not handed over", () => {
  const whole = FIXTURE.source;
  const { version } = whole;
  assert.deepStrictEqual(
    declaration(JSON.stringify(whole)) !== null,
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
    { ...whole, top_zoom: 2 ** 32 + 4 },
    { ...whole, top_zoom: 4.5 },
    { ...whole, format: { ...whole.format, terrain: -1 } },
  ])
    assert.deepStrictEqual(
      declaration(JSON.stringify(bad)),
      null,
      JSON.stringify(bad),
    );
  assert.deepStrictEqual(declaration("not json"), null);
});
