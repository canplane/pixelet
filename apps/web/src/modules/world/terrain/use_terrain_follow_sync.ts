// @pxl/web/modules/world/terrain/use_terrain_follow_sync.ts

import { useEffect } from "react";

import { type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { useCameraStore } from "@/modules/camera/store.ts";
import { useMapStore } from "@/modules/map/store.ts";
import { useTileStore } from "./tile_store.ts";
import { with_ele } from "./use_elevation.ts";

/**
 * Re-samples terrain elevation under the current map_center and snaps it in, if
 * follow_terrain is on. Shared by the tile-arrival retry below and by any one-shot
 * interaction (e.g. the compass's reset-orientation click) that should also leave
 * the view properly grounded instead of only fixing angles/distance.
 */
export function reground_map_center(): void {
  if (!useCameraStore.getState().follow_terrain) return;

  const map_center = useMapStore.getState().map_center;
  if (!map_center) return;

  const probe: PVF3 = pvf3.gather({ i: 0n, f: 0n }, pvf3.X(map_center), pvf3.Y(map_center), 0n);
  const grounded: PVF3 | null = with_ele(probe, probe.i, probe.f);
  if (!grounded) return;

  if (grounded.i !== map_center.i || grounded.f !== map_center.f) {
    useMapStore.getState().set_map_center(grounded);
  }
}

/**
 * Retries grounding map_center whenever new terrain data arrives.
 *
 * The pan handler grounds map_center on every drag step, but if a drag lands
 * the center in an area whose tile hadn't finished loading yet, that attempt
 * falls back to the stale pre-drag height and nothing else re-checks it once
 * the tile does arrive — the center can stay ungrounded indefinitely.
 */
export function useTerrainFollowSync() {
  useEffect(() => {
    reground_map_center();
    return useTileStore.subscribe(reground_map_center);
  }, []);
}
