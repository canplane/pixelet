// script/vector/pxl/shared/spec/space/adapter/3857.ts
//
// EPSG:3857 and 4326 over the world: project, unproject, Z_SCALE, the page
// bbox, the haversine, as bits — for crates/pxl/shared/src/spec/space/adapter/3857.test.rs.
// The transcendental functions are where a libm can disagree in the last
// bit, which is what this vector exists to find.

import * as epsg3857 from "@pxl/shared/spec/space/adapter/3857.ts";
import * as epsg4326 from "@pxl/shared/spec/space/adapter/4326.ts";
import { ivec, type PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { f64_bits, seeded } from "../../../../../../vector.ts";

export function vectors() {
  const rnd = seeded(0x3857);
  const u = () => rnd() / 0x1_0000_0000;
  const lnglat: [number, number, number][] = [[127.0276, 37.4979, 38], [0, 0, 0], [-180, 85.05112878, 0], [179.9999, -85.05112878, 1000], [126.9, 37.5, -12.5]];
  // Past the domain: a longitude wrapped once, twice and exactly onto the
  // seam, and a latitude past Mercator's clamp on either side.
  lnglat.push([190, 89, 0], [-540, -89.9, 5], [360, 85.1, 0], [180, 0, 0], [-180.0000001, -90, 0]);
  for (let k = 0; k < 40; k++) lnglat.push([u() * 360 - 180, u() * 170 - 85, u() * 3000 - 100]);
  return {
    geo: lnglat.map((g) => {
      const m = epsg4326.to_mercator([0, 0, 0], g);
      const back = epsg4326.from_mercator([0, 0, 0], m);
      const p: PVF3 = epsg4326.project({ i: 0n, f: 0n }, g);
      const un = epsg4326.unproject([0, 0, 0], p.i, p.f);
      const p3857: PVF3 = epsg3857.project({ i: 0n, f: 0n }, m[0], m[1], m[2] ?? 0);
      const un3857 = epsg3857.unproject([0, 0, 0], p3857.i, p3857.f);
      return {
        g: g.map(f64_bits),
        mercator: m.map((x) => f64_bits(x ?? 0)),
        from_mercator: back.map((x) => f64_bits(x ?? 0)),
        Y_TO_LAT: f64_bits(epsg4326.Y_TO_LAT(m[1])),
        Z_SCALE: f64_bits(epsg3857.Z_SCALE(m[1])),
        project: { i: p.i, f: p.f },
        unproject: un.map((x) => f64_bits(x ?? 0)),
        project3857: { i: p3857.i, f: p3857.f },
        unproject3857: un3857.map((x) => f64_bits(x ?? 0)),
        dist: f64_bits(epsg4326.dist(g, lnglat[0])),
      };
    }),
    pages: [[5785n, 1836n], [0n, 0n], [-1n, -1n], [8191n, 8191n], [-8192n, -8192n]].map(([x, y]) => {
      const idx = ivec.gather(x, y, 0n);
      const b3857 = epsg3857.get_page_bbox(idx);
      const b4326 = epsg4326.get_page_bbox(idx);
      return { x, y, b3857: b3857.flat().map((v) => f64_bits(v ?? 0)), b4326: b4326.flat().map((v) => f64_bits(v ?? 0)) };
    }),
  };
}
