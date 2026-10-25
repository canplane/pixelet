// @pxl/client/namespace/tile.test.ts

import { assertEquals } from "@std/assert";

import type { BaseLayer } from "@/config/published.spec.ts";
import { tile_url } from "@/config/route.ts";
import { SPHERE } from "@/abi/host.spec.ts";
import { declaration, tile_name, tile_object } from "./tile.ts";
import FIXTURE from "./tile.fixture.json" with { type: "json" };

const DECLARED = declaration(JSON.stringify(FIXTURE.source))!;

Deno.test("tile_object - each kind's tile at each zoom, a shard's own version or not, is the URL the kernel made", () => {
  for (const { kind, subject, object } of FIXTURE.tiles) {
    const k = kind as BaseLayer;
    assertEquals(tile_url(k, tile_object(DECLARED, k, BigInt(subject))!), tile_url(k, object), `${kind} ${subject}`);
  }
  assertEquals(tile_url("earth", tile_object(DECLARED, "earth", BigInt(SPHERE))!), tile_url("earth", FIXTURE.sphere));
});

Deno.test("tile_object - a number that is no object's names none", () => {
  assertEquals(tile_object(DECLARED, "terrain", BigInt(SPHERE)), null);
  assertEquals(tile_name(1n << 64n), null);
  assertEquals(tile_name(1n << 1n), null, "an odd count of digits is no tile's");
});

Deno.test("declaration - one this client cannot read is not handed over", () => {
  const whole = FIXTURE.source;
  const { version } = whole;
  assertEquals(declaration(JSON.stringify(whole)) !== null, true, "the fixture's own is read");
  const { shard: _, ...unsharded } = whole;
  for (
    const bad of [
      { ...whole, version: { ...version, terrain: "a/b" } },
      { ...whole, version: { terrain: version.terrain, place: version.place, division: version.division } },
      { ...whole, shard: { "0103300": { version: { place: "" } } } },
      { ...whole, shard: [] as unknown },
      unsharded,
      { ...whole, top_zoom: 2 ** 32 + 4 },
      { ...whole, top_zoom: 4.5 },
      { ...whole, format: { ...whole.format, terrain: -1 } },
    ]
  ) assertEquals(declaration(JSON.stringify(bad)), null, JSON.stringify(bad));
  assertEquals(declaration("not json"), null);
});
