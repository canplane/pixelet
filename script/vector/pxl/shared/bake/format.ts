// script/vector/pxl/shared/bake/format.ts
//
// The baked terrain packet and the POI page as the TypeScript writes them —
// tags with escapes, index-like keys and undefined values included — and
// what the reader gives back, for crates/pxl/shared/src/bake/format.test.rs.

import * as f from "@pxl/shared/bake/format.ts";
import type { POI } from "@pxl/shared/bake/model/feature.ts";
import { bytes_to_hex } from "@xpute/core/kit/encoding.ts";
import { f64_bits, seeded } from "../../../../vector.ts";

const hex = (s: string) => bytes_to_hex(new TextEncoder().encode(s));

export function vectors() {
  const rnd = seeded(0xba4e);
  const ele = Int16Array.from({ length: 12 }, () => (rnd() % 60000) - 30000);
  const ch = () => Uint8Array.from({ length: 9 }, () => rnd() % 256);
  const ground = { land: ch(), water: ch(), structure: ch(), transport: ch() };
  const terrain = f.encode_terrain(ele, ground);
  const pois: POI[] = [
    { id: "gers-a", type: "poi", category: "food", name: '카페 "한"', center: [1.5, -2.25, 1e-7], tags: { name: "가\nnew", "2": "two", z: undefined, "10": "ten" }, rank: 70000 },
    { id: "gers-b", type: "poi", category: "shop", name: "", center: [0, 0, 0], tags: {}, rank: 3 },
    { id: "", type: "poi", category: "food", name: "tab	in", center: [-0, 1e300, -1e-300], tags: { "": "\\" }, rank: 0 },
  ];
  const page_pkt = f.encode_poi_page(pois);
  const page = new f.PoiPage(page_pkt);
  return {
    ele: Array.from(ele),
    ground: f.GROUND_CH_KEYS.map((k) => bytes_to_hex(ground[k])),
    terrain: bytes_to_hex(terrain),
    decoded: f.GROUND_CH_KEYS.map((k) => bytes_to_hex(f.decode_terrain(terrain).ground[k])),
    pois: pois.map((p) => ({
      id: hex(p.id),
      category: p.category,
      name: hex(p.name),
      center: p.center.map(f64_bits),
      tag_keys: Object.keys(p.tags).map(hex),
      rank: p.rank,
    })),
    page: bytes_to_hex(page_pkt),
    empty: bytes_to_hex(f.encode_empty_poi_page()),
    read: Array.from({ length: page.len }, (_, i) => ({
      category: page.category_of(i),
      tags: hex(JSON.stringify(page.tags_of(i))),
      rank: page.record(i).rank,
    })),
  };
}
