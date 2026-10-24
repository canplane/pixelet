// @pxl/client/namespace/tile.test.ts

import { assertEquals } from "@std/assert";

import type { BaseLayer } from "@/config/published.spec.ts";
import { tileUrl } from "@/config/route.ts";
import { SPHERE } from "@/abi/host.spec.ts";
import { declaration, tile_name, tile_object } from "./tile.ts";
import FIXTURE from "./tile.fixture.json" with { type: "json" };

const DECLARED = declaration(JSON.stringify(FIXTURE.source))!;

Deno.test("tile_object - each kind's tile at each zoom, a shard's own version or not, is the URL the kernel made", () => {
  for (const { kind, subject, object } of FIXTURE.tiles) {
    const k = kind as BaseLayer;
    assertEquals(tileUrl(k, tile_object(DECLARED, k, BigInt(subject))!), tileUrl(k, object), `${kind} ${subject}`);
  }
  assertEquals(tileUrl("earth", tile_object(DECLARED, "earth", BigInt(SPHERE))!), tileUrl("earth", FIXTURE.sphere));
});

Deno.test("tile_object - a number that is no object's names none", () => {
  assertEquals(tile_object(DECLARED, "terrain", BigInt(SPHERE)), null);
  assertEquals(tile_name(1n << 64n), null);
  assertEquals(tile_name(1n << 1n), null, "an odd count of digits is no tile's");
});

Deno.test("declaration - one with a kind unversioned or a version no URL segment is not handed over", () => {
  const { version, shard } = FIXTURE.source;
  for (
    const bad of [
      { version: { ...version, terrain: "a/b" }, shard },
      { version: { terrain: version.terrain, place: version.place, division: version.division }, shard },
      { version, shard: { "0103300": { version: { place: "" } } } },
      { version, shard: [] as unknown },
    ]
  ) assertEquals(declaration(JSON.stringify(bad)), null, JSON.stringify(bad));
  assertEquals(declaration("not json"), null);
});
