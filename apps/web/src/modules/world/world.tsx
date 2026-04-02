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

import { forwardRef, type ReactNode } from "react";
import * as THREE from "three";

import { type FVec, type IVec, projection, type PVF3 } from "@pxl/spec/space/pvf3/mod.ts";
import { L1_IDX_IN_L4, L3_BASE, L3_CENTER } from "@pxl/spec/space/indexer.ts";
import { TO_XY } from "@pxl/spec/space/xy.ts";
import { fill3, floor3, type V3 } from "@pxl/spec/space/v3.ts";

import { U_PER_CELL } from "@pxl/bake/terrain/constants.ts";

import { useMapStore } from "@/modules/map/navigation/map-store.ts";
import { useMapNavigation } from "@/modules/map/navigation/use-map-navigation.ts";
import { get_ele } from "./terrain/use-elevation.ts";

import { AnchorSpace } from "@/modules/world/spatial/anchor/space.tsx";
import { AnchorNode } from "./spatial/anchor/node.tsx";
import { GroundedNode } from "@/modules/world/spatial/anchor/grounded-node.tsx";

import { TerrainLayer } from "@/modules/world/terrain/layer.tsx";
// import { POILayer } from "@/modules/world/poi/layer.tsx";
import { UserMarker } from "./markers/user-marker.tsx";
import { CellHighlighter } from "./markers/cell-highlighter.tsx";
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
  const center: V3 = floor3(v, U_PER_CELL);

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

type WorldProps = {
  children?: ReactNode;
};
export const World = forwardRef<THREE.Group, WorldProps>((props, ref) => {
  const { children } = props;

  const map_center = useMapStore((state) => state.map_center);
  const { map_origin, device_coord } = useMapNavigation();
  if (map_center === null) return null;
  if (device_coord === null) return null;

  return (
    <group ref={ref} name="world">
      {/* 1. Environment layer */}
      <AnchorSpace map_origin={map_origin}>
        <group name="env-layer">
          <TerrainLayer />
          {/* <POILayer /> */}
        </group>

        {/* 2. Content layer */}
        <group name="content-layer">{children}</group>

        {/* 3. Interaction layer */}
        <group name="interaction-layer">
          <CellHighlighter map_center={map_center} map_origin={map_origin} />
          <UserMarker device_coord={device_coord} />
        </group>

        {/* 4. Debug layer */}
        {__IS_DEV__ && <DebugLayer map_center={map_center} map_origin={map_origin} />}
      </AnchorSpace>
    </group>
  );
});
World.displayName = "World";
