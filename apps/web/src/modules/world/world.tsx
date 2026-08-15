// @pxl/web/modules/world/world.tsx

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

import { forwardRef, useMemo, useRef } from "react";
import * as THREE from "three";

import { type FVec, type IVec, projection, type PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L1_IDX_IN_L4, L3_BASE, L3_CENTER } from "@pxl/shared/spec/space/indexer.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";
import { fill3, floor3n, type V3 } from "@pxl/shared/spec/space/v3.ts";

import { U_PER_CELL } from "@pxl/shared/bake/constants.ts";

import { useMapStore } from "@/modules/map/store.ts";
import { useMapNavigation } from "@/modules/map/navigation/use_map_navigation.ts";
import { get_ele } from "./terrain/elevation.ts";
import { useDebugStore } from "@/app/dev/debug_store.ts";

import { AnchorSpace } from "./anchor/space.tsx";
import { AnchorNode } from "./anchor/node.tsx";
import { GroundedNode } from "./anchor/grounded_node.tsx";
import { AnchorRegistry } from "./anchor/registry.ts";

import { useFrameRotation } from "./frame/use_frame_rotation.ts";
import { useFrameTranslation } from "./frame/use_frame_translation.ts";
import { useFrameRefs } from "./frame/context.tsx";
import { get_local_world_origin } from "./frame/coord.ts";

import { LocationPuck } from "./markers/location_puck.tsx";
import { HoverCursor } from "./markers/hover_cursor.tsx";
import { Cube } from "./cube.tsx";

// Unified terrain/object render pipeline — replaces the old split
// TerrainLayer+ObjectLayer (archived: see checkpoint/20260813-before-
// layer-unification for the pre-unification state). CHUNK_STAGE_LAYERS
// lives in modules/world/stack.ts (not inline here) so camera/ray.ts's own
// unified raycast can composite the exact same layer stack instead of a
// second, possibly-drifting registration.
import { ChunkStage } from "./chunk_stage.tsx";
import { CHUNK_STAGE_LAYERS } from "./stack.ts";

type DebugDotProps = {
  i: IVec;
  f?: FVec;

  color: string;
};
function DebugDot(props: DebugDotProps) {
  const { i, f = 0n, color } = props;

  const v: V3 = projection.unproject(fill3(0), L1_IDX_IN_L4(i), f);
  v[2] = get_ele(i, f) ?? 0;
  const center: V3 = floor3n(v, U_PER_CELL);

  return (
    <mesh name="debug-dot" position={center}>
      <sphereGeometry args={[0.5]} />
      <meshBasicMaterial color={color} wireframe />
    </mesh>
  );
}

type DebugLayerProps = {
  map_center: PVF3;
  map_origin: IVec;
};
function DebugLayer(props: DebugLayerProps) {
  const { map_center, map_origin } = props;

  const page_base: IVec = TO_XY(L3_BASE(map_center.i));
  const page_center: IVec = TO_XY(L3_CENTER(map_center.i));

  return (
    <group name="debug-layer">
      <DebugDot i={map_center.i} f={map_center.f} color="red" />

      <DebugDot i={map_origin} color="blue" />

      <GroundedNode i={page_base}>
        <Cube base={[0, U_PER_CELL, 0]} span={1} color="green" />
      </GroundedNode>
      <GroundedNode i={page_center}>
        <Cube base={[U_PER_CELL, 0, 0]} span={1} color="red" />
      </GroundedNode>

      <AnchorNode i={map_center.i} f={map_center.f}>
        <Cube base={fill3(0)} span={1} color="blue" />
      </AnchorNode>
    </group>
  );
}

export const World = forwardRef<THREE.Group>((_props, ref) => {
  const { world_frame_ref, local_world_frame_ref } = useFrameRefs();

  const map_center = useMapStore((state) => state.map_center);
  const { device_coord } = useMapNavigation();
  const show_debug_objects = useDebugStore((state) => state.show_debug_objects);

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
  if (registry.entries.size === 0) registry.cur_map_origin = map_origin;

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

  return (
    <group name="world" ref={ref}>
      <group name="view-frame" ref={view_frame_ref}>
        {__IS_DEV__ && show_debug_objects && (
          <mesh name="view-frame-origin" position={[5, 0, 0]}>
            <sphereGeometry args={[1]} />
            <meshBasicMaterial color="green" wireframe />
          </mesh>
        )}

        <group name="world-frame" ref={world_frame_ref}>
          {__IS_DEV__ && show_debug_objects && (
            <mesh name="world-frame-origin" position={[0, 5, 0]}>
              <sphereGeometry args={[1]} />
              <meshBasicMaterial color="yellow" wireframe />
            </mesh>
          )}

          <group name="local-world-frame" ref={local_world_frame_ref}>
            {__IS_DEV__ && show_debug_objects && (
              <mesh name="local-world-frame-origin" position={[2048, 2048, 0]}>
                <sphereGeometry args={[100]} />
                <meshBasicMaterial color="black" wireframe />
              </mesh>
            )}

            <AnchorSpace registry={registry}>
              <ChunkStage stack={CHUNK_STAGE_LAYERS} />

              <HoverCursor />
              <LocationPuck device_coord={device_coord} />

              {__IS_DEV__ && show_debug_objects && <DebugLayer map_center={map_center} map_origin={map_origin} />}
            </AnchorSpace>
          </group>
        </group>
      </group>
    </group>
  );
});
World.displayName = "World";
