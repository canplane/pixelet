// @pxl/shared/bake/poi_page.test.ts

import { assertEquals } from "@std/assert";

import { fnv1a64_str } from "@xpute/core/kit/hash.ts";

import type { POI } from "./model/feature.ts";
import { encode_empty_poi_page, encode_poi_page, PoiPage } from "./format.ts";

const poi = (i: number, category: string): POI => ({
  id: `gers-${i}`,
  type: "poi",
  category,
  name: `장소 ${i} Place`,
  center: [i * 10.5, -i * 2, 0],
  tags: { addr: `street ${i}`, phone: i % 2 ? "010" : undefined },
  rank: i * 100,
});

Deno.test("poi page - lanes read in place, strings read on demand, every field round-trips", () => {
  const pois = [poi(0, "food"), poi(1, "shop"), poi(2, "food"), poi(3, "park")];
  const page = new PoiPage(encode_poi_page(pois));
  assertEquals(page.len, 4);
  assertEquals(Array.from(page.x), [0, 10.5, 21, 31.5]);
  assertEquals(Array.from(page.y), [0, -2, -4, -6]);
  assertEquals(page.categories, ["food", "shop", "park"]);
  assertEquals(Array.from(page.category), [0, 1, 0, 2]);
  assertEquals(Array.from(page.rank), [0, 100, 200, 300]);
  assertEquals(page.id_hash[2], fnv1a64_str("gers-2"));
  assertEquals(page.name_of(1), "장소 1 Place");
  assertEquals(page.id_of(3), "gers-3");
  assertEquals(page.tags_of(1), { addr: "street 1", phone: "010" });
  assertEquals(page.record(2), { ...pois[2], tags: { addr: "street 2" } });
});

Deno.test("poi page - the lanes are views into the packet, and an empty page has length zero", () => {
  const pkt = encode_poi_page([poi(7, "food")]);
  const page = new PoiPage(pkt);
  assertEquals(page.x.buffer, pkt.buffer);
  assertEquals(page.id_hash.buffer, pkt.buffer);
  const empty = new PoiPage(encode_empty_poi_page());
  assertEquals(empty.len, 0);
  assertEquals(empty.categories, []);
});

Deno.test("poi page - a rank past u16 saturates rather than wrapping", () => {
  const page = new PoiPage(encode_poi_page([{ ...poi(1, "x"), rank: 1_000_000 }]));
  assertEquals(page.rank[0], 0xffff);
});
