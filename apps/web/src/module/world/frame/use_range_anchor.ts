// @pxl/web/module/world/frame/use_range_anchor.ts

import { useMemo } from "react";

import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { useMapStore } from "@/module/map/store.ts";
import { range_anchor_of } from "@/module/world/frame/range.ts";

/**
 * The one anchor every range is measured from: what mounts, what is fetched,
 * what stays resident.
 *
 * Chunk-aligned, so it changes once per 64u of travel rather than every
 * frame, and each consumer re-derives nothing. It is the view centre on the
 * ground, not the camera: residency has to be invariant under rotation, or
 * spinning at a wide zoom would sweep pages in and out of the cache. What the
 * camera itself measures is visibility and tier — see
 * issue/20260912-what-to-render-when.md.
 */
export function useRangeAnchor(): IVec | null {
  const map_center = useMapStore((state) => state.map_center);
  return useMemo(() => (map_center === null ? null : range_anchor_of(map_center.i)), [map_center]);
}
