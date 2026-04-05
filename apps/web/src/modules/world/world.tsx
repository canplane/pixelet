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

import { forwardRef, useRef } from "react";
import * as THREE from "three";

import { type FVec, type IVec, projection, type PVF3 } from "@pxl/spec/space/pvf3/mod.ts";
import { L1_IDX_IN_L4, L3_BASE, L3_CENTER } from "@pxl/spec/space/indexer.ts";
import { TO_XY } from "@pxl/spec/space/xy.ts";
import { fill3, floor3n, type V3 } from "@pxl/spec/space/v3.ts";

import { U_PER_CELL } from "@pxl/bake/terrain/constants.ts";

import { useMapStore } from "@/modules/map/store.ts";
import { useMapNavigation } from "@/modules/map/navigation/use-map-navigation.ts";
import { get_ele } from "./terrain/use-elevation.ts";

import { AnchorSpace } from "./spatial/anchor/space.tsx";
import { AnchorNode } from "./spatial/anchor/node.tsx";
import { GroundedNode } from "./spatial/anchor/grounded-node.tsx";

import { useFrameRefs } from "./frame/context.tsx";
import { get_local_world_origin } from "./frame/coord.ts";

import { TerrainLayer } from "./terrain/layer.tsx";
import { UserMarker } from "./markers/user-marker/bridge.tsx";
import { CellHighlighter } from "./markers/cell-highlighter/bridge.tsx";
import { Block } from "./primitives/block.tsx";

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
        <Block base={[0, U_PER_CELL, 0]} span={1} color="green" />
      </GroundedNode>
      <GroundedNode i={page_center}>
        <Block base={[U_PER_CELL, 0, 0]} span={1} color="red" />
      </GroundedNode>

      <AnchorNode i={map_center.i} f={map_center.f}>
        <Block base={fill3(0)} span={1} color="blue" />
      </AnchorNode>
    </group>
  );
}

export const World = forwardRef<THREE.Group>((_props, ref) => {
  const { world_frame_ref, local_world_frame_ref } = useFrameRefs();

  const map_center = useMapStore((state) => state.map_center);
  const { device_coord } = useMapNavigation();

  const view_frame_ref = useRef<THREE.Group>(null);
  const map_origin: IVec = get_local_world_origin(map_center);

  if (map_center === null) return null;
  if (device_coord === null) return null;

  return (
    <group name="world" ref={ref}>
      <group name="view-frame" ref={view_frame_ref}>
        {__IS_DEV__ && (
          <mesh name="view-frame-origin" position={[5, 0, 0]}>
            <sphereGeometry args={[1]} />
            <meshBasicMaterial color="green" wireframe />
          </mesh>
        )}

        <group name="world-frame" ref={world_frame_ref}>
          {__IS_DEV__ && (
            <mesh name="world-frame-origin" position={[0, 5, 0]}>
              <sphereGeometry args={[1]} />
              <meshBasicMaterial color="yellow" wireframe />
            </mesh>
          )}

          <group name="local-world-frame" ref={local_world_frame_ref}>
            {__IS_DEV__ && (
              <mesh name="local-world-frame-origin" position={[2048, 2048, 0]}>
                <sphereGeometry args={[100]} />
                <meshBasicMaterial color="black" wireframe />
              </mesh>
            )}

            <AnchorSpace map_origin={map_origin}>
              <TerrainLayer />

              <CellHighlighter />
              <UserMarker device_coord={device_coord} />

              {__IS_DEV__ && <DebugLayer map_center={map_center} map_origin={map_origin} />}
            </AnchorSpace>
          </group>
        </group>
      </group>
    </group>
  );
});
World.displayName = "World";
