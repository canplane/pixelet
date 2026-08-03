// @pixelet/shared/bake/model/osm_feature.ts

import type { f64, i32, u32 } from "@pixelet/shared/kit/word.ts";

import type { V3 } from "@pixelet/shared/spec/space/v3.ts";
import type { G3857 } from "@pixelet/shared/spec/space/adapter/3857.ts";

export const enum OSMType {
  NODE = "node",
  WAY = "way",
  RELATION = "relation",
}

/**
 * Shared OSM tag set.
 *
 * Tags are coordinate-system independent and are shared across all ingress,
 * normalization, and bake stages.
 */
export type OSMTags = {
  /** Human-readable feature name. */
  name?: string;

  // ---- Physical / structural tags ----

  /** Building presence or subtype. */
  building?: string;
  /** Building part marker for composite structures. */
  "building:part"?: string;
  /** Number of building levels. */
  "building:levels"?: string;
  /** Explicit minimum building level. */
  "building:min_level"?: string;
  /** Explicit building height in meters. */
  height?: string;
  /** Roof height in meters. */
  "roof:height"?: string;
  /** Road class such as motorway, primary, or residential. */
  highway?: string;
  /** Natural terrain tag such as water, wood, or sand. */
  natural?: string;
  /** Waterway subtype such as river or canal. */
  waterway?: string;
  /** Landuse subtype. */
  landuse?: string;
  /** Leisure area subtype. */
  leisure?: string;
  /** Water body subtype. */
  water?: string;

  // ---- Boundary / area / stacking tags ----

  /** Boundary type such as administrative. */
  boundary?: string;
  /** Administrative level. */
  admin_level?: string;
  /** Marks an area-shaped road feature. */
  "area:highway"?: string;
  /** Bridge marker. */
  bridge?: string;
  /** Tunnel marker. */
  tunnel?: string;
  /** Explicit vertical layer for stacking. */
  layer?: string;
  /** Lane count used for width estimation. */
  lanes?: string;
  /** Explicit width value. */
  width?: string;
  /** Campus marker. */
  campus?: string;

  // ---- POI / place tags ----

  /** Amenity subtype such as cafe, hospital, school, or parking. */
  amenity?: string;
  /** Shop subtype. */
  shop?: string;
  /** Tourism subtype. */
  tourism?: string;
  /** Historic subtype. */
  historic?: string;
  /** Railway subtype. */
  railway?: string;

  /** Allows arbitrary source tags. */
  [key: string]: string | undefined;
};

/**
 * Intermediate raw feature form exchanged between ingress and normalization.
 * Coordinates are expected to be in EPSG:3857 meters.
 * This type preserves source geometry before local page projection.
 */
export type OSMRawFeature = {
  id: f64;
  type: OSMType;
  tags: OSMTags;
  geometry: {
    points: G3857[];
    holes?: G3857[][];
  };
  center?: G3857;
  is_polygon_source?: boolean;
};

/**
 * Derived flags computed during normalization for downstream bake/render logic.
 */
export type OSMExtra = {
  /** Rendering priority score. Higher values indicate more important features. */
  rank: u32;

  /** True when the feature represents ocean/sea fill. */
  is_ocean: boolean;

  /** Parking-area marker. */
  is_parking: boolean;
  /** Institutional-area marker. */
  is_institutional: boolean;
  /** Sports-area marker. */
  is_sports: boolean;
};

/**
 * Final normalized feature form ready for bake/runtime use.
 * Geometry is already projected into local page space and computed metrics are
 * already converted into engine-local units.
 */
export type OSMFeature = {
  /** OSM geometry type. */
  type: OSMType;

  /** OSM identifier carried through normalization. */
  id: f64;
  /** Feature center used for labels, POI markers, and camera focus. */
  center: V3;

  /** Final geometry payload ready for rasterization or line rendering. */
  shape: {
    points: V3[];
    /** Closed-loop marker used to choose fill vs. stroke behavior. */
    is_closed: boolean;
    /** Hole rings assembled from polygon inner members. */
    holes?: V3[][];
  };

  /** Computed metrics derived from source tags. */
  computed: {
    /** Final building height in local cell scale. */
    height: f64;
    /** Final width in local cell scale. */
    width: f64;
    /** Layer used for vertical ordering and offset policy. */
    layer: i32;
  };

  /** Raw source tags kept for metadata and styling decisions. */
  tags: OSMTags;

  /** Final coarse rendering category. */
  category: string;

  /** Derived policy flags used by later bake and render stages. */
  extra: OSMExtra;
};

// ---- Derived object models ----

export type POI = {
  id: string;
  type: "poi";
  category: string;
  name: string;
  center: V3;
  tags: Record<string, string | undefined>;

  /** Final importance rank assigned by policy. */
  rank: u32;
};
