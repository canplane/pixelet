// @pxl/web/module/world/world.tsx

/**
 * Top-level world container for all visual map layers.
 *
 * Structure:
 * 1. Environment: terrain, POI
 * 2. Content: placed voxels and user objects
 * 3. Interaction: highlighters and guide markers
 *
 * Coordinate model:
 * All children inside this group live in the local floating-origin frame.
 * Each layer is responsible for its own local coordinate correction.
 */

import { forwardRef, lazy, Suspense, useMemo, useRef } from "react";
import * as THREE from "three";

import { type FVec, type IVec, projection, type PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L1_IDX_IN_L4, L3_BASE, L3_CENTER, L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";
import { fill3, floor3n, type V3 } from "@pxl/shared/spec/space/v3.ts";

import { U_PER_CELL } from "@pxl/shared/bake/constant.ts";

import { useMapStore } from "@/module/map/store.ts";
import { useMapNavigation } from "@/module/map/navigation/use_map_navigation.ts";
import { get_ele, get_ele_at_local } from "./terrain/elevation.ts";
import { useSettingsStore } from "@/app/setting/store.ts";

import { AnchorSpace } from "./anchor/space.tsx";
import { AnchorNode } from "./anchor/node.tsx";
import { GroundedNode } from "./anchor/grounded_node.tsx";
import { AnchorRegistry } from "./anchor/registry.ts";

import { useFrameRotation } from "./frame/use_frame_rotation.ts";
import { useFrameTranslation } from "./frame/use_frame_translation.ts";
import { useFrameRefs } from "./frame/context.tsx";
import { get_local_world_origin } from "./frame/coord.ts";

import { LocationPuck } from "./marker/location_puck.tsx";
import { HoverCursor } from "./marker/hover_cursor.tsx";
import { DebugBox, DebugPoint, GyroRing, LabeledLine } from "./marker/frame_gizmo.tsx";
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

type DebugDotProps = {
  i: IVec;
  f?: FVec;

  label?: string;
};
function DebugDot(props: DebugDotProps) {
  const { i, f = 0n, label } = props;

  const v: V3 = projection.unproject(fill3(0), L1_IDX_IN_L4(i), f);
  v[2] = get_ele(i, f) ?? 0;
  const center: V3 = floor3n(v, U_PER_CELL);

  return <DebugPoint position={center} label={label} />;
}

type DebugLayerProps = {
  map_center: PVF3;
  map_origin: IVec;
};
function DebugLayer(props: DebugLayerProps) {
  const { map_center, map_origin } = props;

  const page_base: IVec = TO_XY(L3_BASE(map_center.i));
  const page_center: IVec = TO_XY(L3_CENTER(map_center.i));

  // raw_v: true, continuous anchor position (matches what AnchorNode below
  // actually places the map_center point at — no elevation override).
  // quant_v: ground-snapped, floored to the voxel grid — the cell it
  // rounds into (floor(map_center)), one U_PER_CELL cell, drawn as a box
  // because a floored voxel position IS a cell, not a point. map_center
  // itself stays a point (no implied cell) and remains the conceptual
  // center this whole gizmo group is built around — the box merely shows
  // where it floors to, not a recentering.
  const raw_v: V3 = projection.unproject(fill3(0), L1_IDX_IN_L4(map_center.i), map_center.f);
  const gnd_v: V3 = [raw_v[0], raw_v[1], get_ele(map_center.i, map_center.f) ?? 0];
  const quant_v: V3 = floor3n(gnd_v, U_PER_CELL);

  // Same math DebugDot(i=map_origin) uses internally, duplicated here so the
  // line below can reach its exact rendered position — map_origin IS the
  // floating-origin anchor (get_local_world_origin(map_center) in World),
  // labeled anchor_base everywhere in this gizmo group (matches World's own
  // anchor_base further down, a differently-computed but conceptually same
  // point — that one skips unproject/floor3n since local-world-frame's own
  // translation already puts map_origin at local (0,0,*)). Tier naming
  // mirrors raw_v/gnd_v/quant_v above (no raw_v-equivalent — f is
  // always 0n here, so there's no separate pre-elevation point worth naming).
  const anchor_base_gnd_v: V3 = projection.unproject(fill3(0), L1_IDX_IN_L4(map_origin), 0n);
  anchor_base_gnd_v[2] = get_ele(map_origin, 0n) ?? 0;
  const anchor_base_quant_v: V3 = floor3n(anchor_base_gnd_v, U_PER_CELL);

  return (
    <group name="debug-layer">
      <DebugBox base={quant_v} span={U_PER_CELL} label="floor(map_center)" />

      <DebugDot i={map_origin} label="anchor_base (map_origin)" />

      <LabeledLine from={quant_v} to={raw_v} />

      <LabeledLine from={quant_v} to={anchor_base_quant_v} label="floor(map_center) <-> anchor_base (map_origin)" dashed />

      <GroundedNode i={page_base}>
        <DebugBox base={fill3(0)} span={1} label="page_base" />
      </GroundedNode>
      <GroundedNode i={page_center}>
        <DebugBox base={fill3(0)} span={1} label="page_center" />
      </GroundedNode>

      <AnchorNode i={map_center.i} f={map_center.f}>
        <DebugPoint position={[0, 0, 0]} label="map_center" />
      </AnchorNode>
    </group>
  );
}

export const World = forwardRef<THREE.Group>((_props, ref) => {
  const { world_frame_ref, local_world_frame_ref } = useFrameRefs();

  const map_center = useMapStore((state) => state.map_center);
  const { device_coord } = useMapNavigation();
  const show_debug_objects = useSettingsStore((state) => state.show_debug_objects);

  const view_frame_ref = useRef<THREE.Group>(null);
  const map_origin: IVec = get_local_world_origin(map_center);

  // owned here (not by AnchorSpace) so useFrameTranslation can rebase it in
  // the same callback that snaps local-world-frame's position — see the doc
  // comment on useFrameTranslation's registry param.
  const registry = useMemo(() => new AnchorRegistry(), []);

  // map_center starts out null, so the registry above is first created with
  // cur_map_origin stuck at 0n (its own field default) — the real origin
  // isn't known until map_center resolves. Once any anchor has linked,
  // cur_map_origin may only change through shift_map_origin (which also
  // repositions those existing anchors — see useFrameTranslation's own
  // callback), but while entries is still empty there's nothing to
  // reposition, so it's safe to keep this synced directly here, before any
  // AnchorNode's layout effect can link() using a stale origin.
  if (registry.size === 0) registry.cur_map_origin = map_origin;

  useFrameRotation({ frame_ref: world_frame_ref });
  useFrameTranslation({
    frame_ref: local_world_frame_ref,
    map_center,
    map_origin,
    registry,
  });

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
    <group name="world" ref={ref}>
      <group name="view-frame" ref={view_frame_ref}>
        {__IS_DEV__ && show_debug_objects && (
          <>
            {
              /* view-frame is never rotated by useFrameRotation (only its
              world-frame child is) — a marker here stays fixed relative to
              the camera/screen regardless of azimuth, i.e. "my right"
              regardless of how the world has been rotated underneath. */
            }
            <GyroRing radius={4} vertex={[4, 0, 0]} label="view-frame (4,0,0)" />
            <DebugPoint position={[0, 0, 4]} label="view-frame (0,0,4)" />
          </>
        )}

        <group name="world-frame" ref={world_frame_ref}>
          {__IS_DEV__ && show_debug_objects && (
            // world-frame's own rotation.z is driven straight from camera
            // azimuth (useFrameRotation) — a marker here rotates exactly
            // opposite the camera's orbit, so it stays pointed at a fixed
            // geographic bearing (north) on screen no matter how the camera
            // orbits.
            <GyroRing radius={4} vertex={[0, 4, 0]} label="north (world-frame, azimuth-locked)" show_radius />
          )}

          <group name="local-world-frame" ref={local_world_frame_ref}>
            {__IS_DEV__ && show_debug_objects && (() => {
              // This marker's children are positioned relative to
              // map_origin — the same convention DebugDot/DebugLayer use
              // (map_origin itself renders at (0,0,0)) — so (2048,2048) is
              // not an arbitrary offset: L4_RANGE is 4096 and L4_CENTER =
              // L4_BASE | (L4_RANGE >> 1), so the center of the L4 tile that
              // owns the current floating origin sits at exactly
              // (2048,2048) relative to map_origin, always, by construction
              // — not a "just visible" nudge. Z is elevation-snapped the
              // same way every other DebugLayer marker is (get_ele_at_local
              // — the local-coordinate-relative sibling of get_ele used
              // here since this point, unlike DebugLayer's own markers,
              // isn't itself a real PVF3 address, only a local offset), so
              // it sits on the ground instead of floating at whatever
              // height map_origin's own Z happens to be.
              const anchor_center: V3 = [2048, 2048, get_ele_at_local(TO_XY(L3_IDX(map_origin)), 2048, 2048) ?? 0];
              const anchor_base: V3 = [0, 0, get_ele(map_origin, 0n) ?? 0];
              return (
                <>
                  <DebugPoint position={anchor_center} label="anchor_center" />
                  <LabeledLine from={anchor_base} to={anchor_center} label="anchor_center <-> anchor_base (map_origin)" dashed />
                </>
              );
            })()}

            <AnchorSpace registry={registry}>
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
            </AnchorSpace>
          </group>
        </group>
      </group>
    </group>
  );
});
World.displayName = "World";
