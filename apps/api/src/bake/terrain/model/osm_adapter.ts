// @pxl/api/bake/terrain/model/osm_adapter.ts

/**
 * OSM ingress adapter for WKB-based terrain source rows.
 * Handles the canonical page terrain source payload shape
 * (WKB geometry + hstore text tags).
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import type { U8Array } from "@xpute/core/kit/array.ts";

import type { G3857 } from "@pxl/shared/spec/space/adapter/3857.ts";

import type { CoastRow, FeatRow } from "@pxl/shared/bake/format.ts";
import { type OSMRawFeature, type OSMTags, OSMType } from "@pxl/shared/bake/model/osm_feature.ts";

type WKBGeometry = {
  points: G3857[];
  holes: G3857[][];
  is_polygon_source: boolean;
};

// -----------------------------------------------------------------------------
// WKB ingress for page terrain source rows
// -----------------------------------------------------------------------------

export function from_feat_row(row: FeatRow): OSMRawFeature[] {
  const tags: OSMTags = row.tags_hstore ? parse_hstore(row.tags_hstore) : ({} as OSMTags);
  const geoms: WKBGeometry[] = wkb_to_geometries(row.way_wkb);
  const center: G3857 = wkb_to_point_or_null(row.center_wkb) ?? (geoms[0]?.points?.[0] ?? [0, 0, 0]);

  const out: OSMRawFeature[] = new Array(geoms.length);
  for (let i = 0; i < geoms.length; i++) {
    const g = geoms[i];
    out[i] = {
      id: Number(row.osm_id),
      type: g.points.length === 1 ? OSMType.NODE : OSMType.WAY,
      tags,
      geometry: g,
      center,
      is_polygon_source: g.is_polygon_source,
    };
  }
  return out;
}

export function from_coast_row(row: CoastRow): OSMRawFeature[] {
  if (row.covers_page) return []; // clipped full-page coast polygon => bogus sea fill candidate

  const geoms: WKBGeometry[] = wkb_to_geometries(row.geom_wkb);
  const out: OSMRawFeature[] = new Array(geoms.length);
  for (let i = 0; i < geoms.length; i++) {
    const g = geoms[i];
    out[i] = {
      id: Number(row.id),
      type: OSMType.WAY,
      tags: {
        natural: "ocean",
        source: "osm_coastline",
      },
      geometry: g,
      center: g.points[0] ?? [0, 0, 0],
      is_polygon_source: true,
    };
  }
  return out;
}

// -----------------------------------------------------------------------------
// hstore raw string -> tag dictionary
// -----------------------------------------------------------------------------

function parse_hstore(hstore_str: string): OSMTags {
  const result: Record<string, string> = {};
  const pattern = /"(.+?)"=>"(.*?)"/g;
  let match;
  while ((match = pattern.exec(hstore_str)) !== null) {
    result[match[1]] = match[2];
  }
  // hstore guarantees only string key/value pairs.
  // Later policy/normalization stages interpret the subset relevant to OSMTags.
  return result as unknown as OSMTags;
}

// -----------------------------------------------------------------------------
// Minimal internal WKB decode.
// Supports: Point, LineString, Polygon, MultiPolygon.
// Accepts EWKB flags (Z/SRID) and ignores SRID/Z because the pipeline assumes 3857.
// -----------------------------------------------------------------------------

function wkb_to_point_or_null(wkb: U8Array): G3857 | null {
  const dv = new DataView(wkb.buffer, wkb.byteOffset, wkb.byteLength);
  let o: u32 = 0;
  if (dv.byteLength < 1 + 4) return null;
  const le = dv.getUint8(o) === 1;
  o += 1;
  const t = dv.getUint32(o, le);
  o += 4;

  const has_srid = (t & 0x20000000) !== 0;
  const has_z = (t & 0x80000000) !== 0;
  const base = t & 0xff;

  if (has_srid) o += 4;
  if (base !== 1) return null;
  if (dv.byteLength < o + 16) return null;
  const x = dv.getFloat64(o, le);
  o += 8;
  const y = dv.getFloat64(o, le);
  o += 8;
  if (has_z && dv.byteLength >= o + 8) o += 8;
  return [x, y, 0];
}

function wkb_to_geometries(wkb: U8Array): WKBGeometry[] {
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
