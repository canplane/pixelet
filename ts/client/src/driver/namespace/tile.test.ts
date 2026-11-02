// @pxl/client/driver/namespace/tile.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import type { BaseLayer } from "./publication.spec.ts";
import { tile_url } from "@/driver/namespace/route.ts";
import { SPHERE } from "@/abi/host.spec.ts";
import { declaration } from "./source.ts";
import { tile_name, tile_object } from "./tile.ts";
import FIXTURE from "./tile.fixture.json" with { type: "json" };

const DECLARATION = declaration(JSON.stringify(FIXTURE.source))!;

test("tile_object - each kind's tile at each zoom, a shard's own version or not, is the URL the kernel made", () => {
  for (const { kind, subject, object } of FIXTURE.tiles) {
    const k = kind as BaseLayer;
    assert.deepStrictEqual(
      tile_url(k, tile_object(DECLARATION, k, BigInt(subject))!),
      tile_url(k, object),
      `${kind} ${subject}`,
    );
  }
  assert.deepStrictEqual(
    tile_url("earth", tile_object(DECLARATION, "earth", BigInt(SPHERE))!),
    tile_url("earth", FIXTURE.sphere),
  );
});

test("tile_object - a number that is no object's names none", () => {
  assert.deepStrictEqual(
    tile_object(DECLARATION, "terrain", BigInt(SPHERE)),
    null,
  );
  assert.deepStrictEqual(tile_name(1n << 64n), null);
  assert.deepStrictEqual(
    tile_name(1n << 1n),
    null,
    "an odd count of digits is no tile's",
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
