// @pxl/web/module/world/world.tsx

/**
 * Top-level world container for all map layers.
 *
 * Structure:
 * 1. Environment: terrain, POI
 * 2. Content: placed voxels and user objects
 * 3. Interaction: highlighters and guide markers
 *
 * Coordinate model: every layer places what it has in the root frame
 * (module/world/view.ts), the map's azimuth and the floating origin's
 * translation, which the view's damping steps write. Nothing here renders
 * DOM but the dev gizmos' labels: each layer tells the module what it has.
 */

import { lazy, Suspense } from "react";

import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L1_IDX_IN_L4, L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";
import { fill3, type V3 } from "@pxl/shared/spec/space/v3.ts";
import { projection } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { useMapStore } from "@/module/map/store.ts";
import { useMapNavigation } from "@/module/map/navigation/use_map_navigation.ts";
import { useSettingsStore } from "@/app/setting/store.ts";

import { useViewRotation, useViewTranslation } from "./frame/use_view_frame.ts";
import { get_local_world_origin } from "./frame/coord.ts";

import { LocationPuck } from "./marker/location_puck.tsx";
import { HoverCursor } from "./marker/hover_cursor.tsx";
import { DebugLayer } from "./marker/debug_layer.tsx";
import { PoiLayer } from "./poi/poi_layer.tsx";

// CHUNK_STAGE_LAYERS lives in module/world/stack.ts (not inline here) so
// camera/ray.ts's own raycast can composite the exact same layer stack
// instead of a second, possibly-drifting registration.
import { ChunkStage, LodSchedulerProvider } from "./chunk_stage.tsx";
// The layers teach the module their tables when imported; the chunks it
// builds read them.
import "./stack.ts";

// Dynamic rather than a plain top-level import so the character/pet model
// code (GLTFLoader, skinning, animation mixer — otherwise unused anywhere
// else in the app) lands in its own chunk instead of bloating the main one.
// This lazy() still uses Suspense, but only once — for the code chunk
// itself, at initial mount — not per-frame like the model-loading inside
// Companion used to (see use_character_model.ts's own doc comment for why
// that had to stop being Suspense-based entirely).
const Companion = lazy(() => import("./character/companion.tsx").then((m) => ({ default: m.Companion })));

export function World() {
  const map_center = useMapStore((state) => state.map_center);
  const { device_coord } = useMapNavigation();
  const show_debug_objects = useSettingsStore((state) => state.show_debug_objects);

  const map_origin: IVec = get_local_world_origin(map_center);

  useViewRotation();
  useViewTranslation({ map_center, map_origin });

  // device_coord is deliberately NOT gated here the way map_center is —
  // map_center can be seeded from a fallback (see use_map_bootstrap.ts) even
  // when there's no real device fix at all, and the terrain/world itself
  // should still render in that case. LocationPuck itself now holds its own
  // last-known device_coord across a transient null (a GPS blip shouldn't
  // unmount/remount it — see its own doc comment), so it no longer needs to
  // be conditionally mounted here either.
  if (map_center === null) return null;

  // Same computation DebugLayer's own raw_v uses — map_center's position in
  // the local floating-origin frame. Companion needs this (not the fixed
  // L4-tile-center point DebugLayer's anchor_center marker uses) so it sits
  // wherever the camera actually is, not up to 2048u/~4.9km away.
  const local_map_center: V3 = projection.unproject(fill3(0), L1_IDX_IN_L4(map_center.i), map_center.f);

  return (
    <>
      <LodSchedulerProvider>
        <ChunkStage />
      </LodSchedulerProvider>

      <HoverCursor />
      <LocationPuck device_coord={device_coord} />
      <PoiLayer />

      <Suspense fallback={null}>
        <Companion
          origin_page_idx_xy={TO_XY(L3_IDX(map_origin))}
          map_origin={map_origin}
          center_x={local_map_center[0]}
          center_y={local_map_center[1]}
          center_z={local_map_center[2]}
        />
      </Suspense>

      {__IS_DEV__ && show_debug_objects && <DebugLayer map_center={map_center} map_origin={map_origin} />}
    </>
  );
}
