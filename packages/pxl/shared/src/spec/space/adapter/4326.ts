// @pxl/shared/spec/space/adapter/4326.ts

/**
 * EPSG:4326 <-> EPSG:3857 Projection Utilities
 *
 * ROLE
 * ----
 * Pure mathematical projection between:
 * - EPSG:4326 (WGS84 longitude/latitude)
 * - EPSG:3857 (Web Mercator meters)
 *
 * NOTES
 * -----
 * - This layer performs no canonical-unit scaling by itself.
 * - Scaling into G64 / canonical `u` space is handled by the 3857 adapter layer.
 * - Rendering conventions and axis remapping
 * belong to higher layers.
 *
 * @module epsg4326
 */

import type { f64 } from "@xpute/core/kit/word.ts";

import type { FVec, IVec, PVF3 } from "../pvf3/mod.ts";

import type { G3857 } from "./3857.ts";
import * as epsg3857 from "./3857.ts";

/**
 * EPSG:4326 (WGS84) geodetic coordinates.
 */
export type G4326 = [
  lng: f64,
  lat: f64,
  alt?: f64, // meters above sea level (optional)
];

/**
 * Latitude limits for Web Mercator.
 * Beyond these bounds, Y diverges to infinity.
 */
const LAT_MAX: f64 = 85.05112878;
const LAT_MIN: f64 = -85.05112878;

/**
 * Earth radius used by EPSG:3857 (meters).
 */
const EARTH_RADIUS: f64 = 6378137;

const DEG_TO_RAD: f64 = Math.PI / 180;
const RAD_TO_DEG: f64 = 180 / Math.PI;

const G3857_REG: G3857 = [0, 0, 0];

/**
 * Normalizes longitude into [-180, 180).
 */
const NORMALIZE_LNG = (lng: f64): f64 => ((((lng + 180) % 360) + 360) % 360) - 180;

/**
 * Clamps latitude into the valid Web Mercator domain.
 */
const CLAMP_LAT = (lat: f64): f64 => Math.max(LAT_MIN, Math.min(LAT_MAX, lat));

/**
 * Longitude (degrees) -> Web Mercator X (meters).
 */
const LNG_TO_X = (lng: f64): f64 => NORMALIZE_LNG(lng) * DEG_TO_RAD * EARTH_RADIUS;

/**
 * Latitude (degrees) -> Web Mercator Y (meters).
 */
const LAT_TO_Y = (lat: f64): f64 => {
  const lat_rad = CLAMP_LAT(lat) * DEG_TO_RAD;
  return Math.log(Math.tan(Math.PI / 4 + lat_rad / 2)) * EARTH_RADIUS;
};

/**
 * Web Mercator X (meters) -> longitude (degrees).
 */
const X_TO_LNG = (x: f64): f64 => NORMALIZE_LNG((x / EARTH_RADIUS) * RAD_TO_DEG);

/**
 * Web Mercator Y (meters) -> latitude (degrees).
 */
export const Y_TO_LAT = (y: f64): f64 => {
  const lat_rad: f64 = 2 * Math.atan(Math.exp(y / EARTH_RADIUS)) - Math.PI / 2;
  return lat_rad * RAD_TO_DEG;
};

// -----------------------------------------------------------------------------
// Core Projection (Math Only)
// -----------------------------------------------------------------------------

/**
 * Projects EPSG:4326 -> EPSG:3857.
 *
 * - Longitude is normalized (cyclic).
 * - Latitude is clamped (finite Mercator domain).
 * - Altitude is passed through unchanged when present.
 */
export const to_mercator = (dst: G3857, src: G4326): G3857 => {
  dst[0] = LNG_TO_X(src[0]);
  dst[1] = LAT_TO_Y(src[1]);
  dst[2] = src[2];
  return dst;
};

/**
 * Unprojects EPSG:3857 -> EPSG:4326.
 *
 * - Longitude is normalized for canonical output.
 * - Altitude is passed through unchanged.
 */
export const from_mercator = (dst: G4326, src: G3857): G4326 => {
  dst[0] = X_TO_LNG(src[0]);
  dst[1] = Y_TO_LAT(src[1]);
  dst[2] = src[2];
  return dst;
};

/**
 * EPSG:4326 -> G64
 * 4326 geodetic -> 3857 planar meters -> G64 stored in PVF3
 */
export const project = (dst: PVF3, src: G4326): PVF3 => {
  // 1. LonLat -> Meters (Store in static buffer)
  to_mercator(G3857_REG, src);
  // 2. Meters -> Lattice (Write to dst)
  epsg3857.project(dst, G3857_REG[0], G3857_REG[1], G3857_REG[2]);
  return dst;
};

/**
 * G64 -> EPSG:4326
 * G64 stored in PVF3 -> 3857 planar meters -> 4326 geodetic
 */
export const unproject = (out: G4326, i: IVec, f: FVec): G4326 => {
  // 1. Lattice -> Meters (Store in static buffer)
  epsg3857.unproject(G3857_REG, i, f);
  // 2. Meters -> LonLat (Write to out)
  from_mercator(out, G3857_REG);
  return out;
};

// ---- Distance ----

/**
 * Computes great-circle distance (meters) using the haversine formula.
 *
 * - Ignores altitude.
 * - Measures ground distance only.
 */
export const dist = (a: G4326, b: G4326): f64 => {
  const d_lng: f64 = (b[0] - a[0]) * DEG_TO_RAD;
  const d_lat: f64 = (b[1] - a[1]) * DEG_TO_RAD;

  const sa: f64 = Math.sin(d_lat / 2);
  const sb: f64 = Math.sin(d_lng / 2);

  const h: f64 = sa * sa + Math.cos(a[1] * DEG_TO_RAD) * Math.cos(b[1] * DEG_TO_RAD) * sb * sb;

  return 2 * EARTH_RADIUS * Math.atan2(Math.sqrt(h), Math.sqrt(1 - h));
};

// ---- Bounding Box ----

export type BBox4326 = [min: G4326, max: G4326];

export const get_page_bbox = (page_idx: IVec): BBox4326 => {
  const [min_m, max_m] = epsg3857.get_page_bbox(page_idx);

  const min: G4326 = [0, 0, 0];
  const max: G4326 = [0, 0, 0];
  from_mercator(min, min_m);
  from_mercator(max, max_m);

  // from_mercator normally canonicalizes +180° to -180°. A bbox maximum is
  // an exclusive interval endpoint rather than a coordinate, so preserve
  // +180° for the final eastern page instead of inverting the min/max pair.
  if (max_m[0] > min_m[0] && max[0] < min[0]) max[0] = 180;

  return [min, max];
};
