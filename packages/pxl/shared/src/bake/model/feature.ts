// @pxl/shared/bake/model/feature.ts

import type { f64, i32, u32 } from "@xpute/core/kit/word.ts";

import type { V3 } from "@pxl/shared/spec/space/v3.ts";
import type { G3857 } from "@pxl/shared/spec/space/adapter/3857.ts";
import type { CellConfig } from "@pxl/shared/bake/format.ts";

/**
 * Synthetic tag set attached to coastline rows (coast_adapter.ts's
 * from_coast_row) — coast_policy.ts's own IS_COAST check reads `natural`/
 * `source` off this shape.
 */
export type CoastTags = {
  natural?: string;
  source?: string;
  [key: string]: string | undefined;
};

/**
 * Raw coastline row form exchanged between coast_adapter.ts and
 * normalizer.ts. Coordinates are expected to be in EPSG:3857 meters. This
 * type preserves source geometry before local page projection.
 */
export type CoastRawFeature = {
  id: string;
  tags: CoastTags;
  geometry: {
    points: G3857[];
    holes?: G3857[][];
    is_polygon_source?: boolean;
  };
  center?: G3857;
};

/**
 * Derived flags computed during normalization for downstream bake/render logic.
 */
export type FeatureExtra = {
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
  /** True when the feature is a tunnel and should not be rasterized. */
  is_tunnel: boolean;
};

/**
 * Final normalized feature form ready for bake/runtime use, shared by every
 * source provider (coastline, Overture Buildings/Transportation/Base).
 * Geometry is already projected into local page space and computed metrics
 * are already converted into engine-local units.
 */
export type Feature = {
  /** Source-provider identifier (Overture GERS id, or coast_adapter.ts's synthetic row id) carried through normalization. */
  id: string;
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

  /** Final coarse rendering category. */
  category: string;

  /** Derived policy flags used by later bake and render stages. */
  extra: FeatureExtra;

  /** Target ground channel + cell type, resolved by the source provider's own policy. Null when the feature should not be rasterized. */
  cell_cfg: CellConfig | null;
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
