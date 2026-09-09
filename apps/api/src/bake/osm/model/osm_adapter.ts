// @pxl/api/bake/osm/model/osm_adapter.ts

/**
 * Coastline ingress adapter for WKB-based terrain source rows. Despite the
 * filename, `from_coast_row`'s own input is Overture-derived now too —
 * repository.ts's coast query computes it from overture_base_water's
 * `ocean` subtype, not osm_coastline_polygons (see that query's own
 * comment for why/how). Left under bake/osm/ rather than moved: its WKB
 * decoder (wkb_to_geometries/WKBGeometry) is still shared with
 * overture_adapter.ts, and a rename here is cosmetic, not a correctness
 * question — revisit if bake/osm/ ever needs to mean something again.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import type { U8Array } from "@xpute/core/kit/array.ts";

import type { G3857 } from "@pxl/shared/spec/space/adapter/3857.ts";

import type { CoastRow } from "@pxl/shared/bake/format.ts";
import type { OSMRawFeature } from "@pxl/shared/bake/model/osm_feature.ts";

export type WKBGeometry = {
  points: G3857[];
  holes: G3857[][];
  is_polygon_source: boolean;
};

// -----------------------------------------------------------------------------
// WKB ingress for page terrain source rows
// -----------------------------------------------------------------------------

export function from_coast_row(row: CoastRow): OSMRawFeature[] {
  if (row.covers_page) return []; // clipped full-page coast polygon => bogus sea fill candidate

  const geoms: WKBGeometry[] = wkb_to_geometries(row.geom_wkb);
  const out: OSMRawFeature[] = new Array(geoms.length);
  for (let i = 0; i < geoms.length; i++) {
    const g = geoms[i];
    out[i] = {
      id: row.id,
      tags: {
        natural: "ocean",
        source: "overture_ocean",
      },
      geometry: g,
      center: g.points[0] ?? [0, 0, 0],
    };
  }
  return out;
}

// -----------------------------------------------------------------------------
// Minimal internal WKB decode.
// Supports: Point, LineString, Polygon, MultiPolygon.
// Accepts EWKB flags (Z/SRID) and ignores SRID/Z because the pipeline assumes 3857.
// -----------------------------------------------------------------------------

export function wkb_to_geometries(wkb: U8Array): WKBGeometry[] {
  if (!wkb.byteLength) return [];
  const dv = new DataView(wkb.buffer, wkb.byteOffset, wkb.byteLength);
  const [geoms] = wkb_read_any(dv, 0);
  return geoms;
}

function wkb_read_any(dv: DataView, off: u32): [WKBGeometry[], u32] {
  let o: u32 = off;
  const le = dv.getUint8(o) === 1;
  o += 1;
  const t = dv.getUint32(o, le);
  o += 4;

  const has_srid = (t & 0x20000000) !== 0;
  const has_z = (t & 0x80000000) !== 0;
  const base = t & 0xff;
  if (has_srid) o += 4;

  switch (base) {
    case 1: { // Point
      const x = dv.getFloat64(o, le);
      o += 8;
      const y = dv.getFloat64(o, le);
      o += 8;
      if (has_z) o += 8;
      return [[{ points: [[x, y, 0]], holes: [], is_polygon_source: false }], o];
    }
    case 2: { // LineString
      const n = dv.getUint32(o, le);
      o += 4;
      const pts: G3857[] = new Array(n);
      for (let i = 0; i < n; i++) {
        const x = dv.getFloat64(o, le);
        o += 8;
        const y = dv.getFloat64(o, le);
        o += 8;
        if (has_z) o += 8;
        pts[i] = [x, y, 0];
      }
      return [[{ points: pts, holes: [], is_polygon_source: false }], o];
    }
    case 3: { // Polygon
      const nr = dv.getUint32(o, le);
      o += 4;
      if (nr === 0) return [[{ points: [], holes: [], is_polygon_source: true }], o];

      // ring0 = outer, ring1.. = holes
      const outer = wkb_read_ring(dv, o, le, has_z);
      o = outer[1];
      const holes: G3857[][] = [];
      for (let r = 1; r < nr; r++) {
        const h = wkb_read_ring(dv, o, le, has_z);
        o = h[1];
        holes.push(h[0]);
      }
      return [[{ points: outer[0], holes, is_polygon_source: true }], o];
    }
    case 6: { // MultiPolygon
      const np = dv.getUint32(o, le);
      o += 4;
      const out: WKBGeometry[] = [];
      for (let i = 0; i < np; i++) {
        const [g, o2] = wkb_read_any(dv, o);
        o = o2;
        // each element is Polygon => g[0] should exist
        for (let k = 0; k < g.length; k++) {
          // force polygon semantics
          g[k].is_polygon_source = true;
          out.push(g[k]);
        }
      }
      return [out, o];
    }
    default:
      // unsupported => empty
      return [[], o];
  }
}

function wkb_read_ring(dv: DataView, off: u32, le: boolean, has_z: boolean): [G3857[], u32] {
  let o: u32 = off;
  const n = dv.getUint32(o, le);
  o += 4;
  const pts: G3857[] = new Array(n);
  for (let i = 0; i < n; i++) {
    const x = dv.getFloat64(o, le);
    o += 8;
    const y = dv.getFloat64(o, le);
    o += 8;
    if (has_z) o += 8;
    pts[i] = [x, y, 0];
  }
  return [pts, o];
}
