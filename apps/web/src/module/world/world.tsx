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
 * translation, which the view's damping steps write. Three's scene holds
 * only the dev gizmos and the decor hit test now, under the anchor root,
 * whose matrix follows the view.
 */

import { lazy, Suspense, useMemo } from "react";
import { createPortal, useThree } from "@react-three/fiber";

import { type FVec, type IVec, projection, type PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L1_IDX_IN_L4, L3_BASE, L3_CENTER, L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";
import { fill3, floor3n, type V3 } from "@pxl/shared/spec/space/v3.ts";

import { U_PER_CELL } from "@pxl/shared/bake/constant.ts";

import { useMapStore } from "@/module/map/store.ts";
import { useMapNavigation } from "@/module/map/navigation/use_map_navigation.ts";
import { get_ele } from "./terrain/elevation.ts";
import { useSettingsStore } from "@/app/setting/store.ts";

import { AnchorSpace } from "./anchor/space.tsx";
import { AnchorNode } from "./anchor/node.tsx";
import { GroundedNode } from "./anchor/grounded_node.tsx";
import { AnchorRegistry } from "./anchor/registry.ts";

import { useViewRotation, useViewTranslation } from "./frame/use_view_frame.ts";
import { get_local_world_origin } from "./frame/coord.ts";
import { view } from "./view.ts";
import { useTick } from "./sched/tick.ts";

import { LocationPuck } from "./marker/location_puck.tsx";
import { HoverCursor } from "./marker/hover_cursor.tsx";
import { DebugBox, DebugPoint, LabeledLine } from "./marker/frame_gizmo.tsx";
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

/**
 * Three's camera and the anchor root follow the view, so the gizmos and
 * the decor hit test stand where the renderer draws. After every step that
 * moves the view, before three renders.
 */
function ThreeMirror({ registry }: { registry: AnchorRegistry }) {
  const camera = useThree((state) => state.camera);
  useTick("cosmetic", () => {
    camera.matrixAutoUpdate = false;
    camera.matrixWorld.fromArray(view.inv_view_matrix);
    camera.matrix.copy(camera.matrixWorld);
    camera.matrixWorldInverse.fromArray(view.view_matrix);
    camera.projectionMatrix.fromArray(view.proj);
    camera.projectionMatrixInverse.copy(camera.projectionMatrix).invert();
    // The root's matrix is written by hand, so three has to be told the
    // world matrices under it are stale, or it composes nothing.
    const root = registry.root;
    root.matrixAutoUpdate = false;
    root.matrix.fromArray(view.root);
    root.matrixWorldNeedsUpdate = true;
  }, "three_mirror");
  return null;
}

export function World() {
  const map_center = useMapStore((state) => state.map_center);
  const { device_coord } = useMapNavigation();
  const show_debug_objects = useSettingsStore((state) => state.show_debug_objects);

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

  useViewRotation();
  useViewTranslation({ map_center, map_origin, registry });

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
    <AnchorSpace registry={registry}>
      <ThreeMirror registry={registry} />
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

      {/* Into the anchor root, whose matrix is the root frame's. */}
      {__IS_DEV__ && show_debug_objects && createPortal(<DebugLayer map_center={map_center} map_origin={map_origin} />, registry.root)}
    </AnchorSpace>
  );
}
