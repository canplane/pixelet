// @pxl/web/modules/world/world.tsx

/**
 * 지도의 모든 시각적 레이어(지형, 마커, 오브젝트)를 통합하는 최상위 컨테이너입니다.
 * * * 🏗️ [Structure]
 * 1. Environment: terrain, POI
 * 2. Content: 복셀, 설치물 (User Content)
 * 3. Interaction: 하이라이터, 마커 (UI/UX)
 * * * 🌐 [Coordinate System]
 * 이 그룹 내의 모든 자식 요소는 Floating Origin 시스템에 의해
 * 자신의 로컬 좌표를 스스로 보정(Self-Correction)해야 합니다.
 */

import { forwardRef, type ReactNode } from "react";
import * as THREE from "three";

import { type FVec, type IVec, projection, type PVF3 } from "@pxl/spec/space/pvf3/mod.ts";
import { to3 } from "@pxl/spec/space/adapter/three.ts";
import { L1_IDX_IN_L4, L3_BASE, L3_CENTER } from "@pxl/spec/space/indexer.ts";
import { TO_XY } from "@pxl/spec/space/xy.ts";
import { floor3, type V3 } from "@pxl/spec/space/v3.ts";

import { U_PER_CELL } from "@pxl/bake/terrain/constants.ts";

import { useMapStore } from "@/modules/map/navigation/map-store.ts";
import { useMapNavigation } from "@/modules/map/navigation/use-map-navigation.ts";

import { AnchorSpace } from "@/modules/world/spatial/anchor/space.tsx";
import { AnchorNode } from "./spatial/anchor/node.tsx";
import { GroundedNode } from "@/modules/world/spatial/anchor/grounded-node.tsx";

import { TerrainLayer } from "@/modules/world/terrain/layer.tsx";
// import { POILayer } from "@/modules/world/poi/layer.tsx";
import { UserMarker } from "./markers/user-marker.tsx";
import { CellHighlighter } from "./markers/cell-highlighter.tsx";
import { Block } from "./primitives/block.tsx";
import { get_ele } from "./terrain/use-elevation.ts";

type DotProps = {
  i: IVec;
  f?: FVec;

  color: string;
};
function Dot(props: DotProps) {
  const { i, f = 0n, color } = props;

  const v: V3 = projection.unproject([0, 0, 0], L1_IDX_IN_L4(i), f);
  v[2] = get_ele(i, f) ?? 0;
  const center: V3 = floor3(v, [1, 1, 1]);

  return (
    <mesh name="dot" position={to3(center)}>
      <sphereGeometry args={[0.5]} />
      <meshBasicMaterial color={color} wireframe />
    </mesh>
  );
}

type TestNodeProps = {
  map_center: PVF3;
  map_origin: IVec;
};
function TestNode(props: TestNodeProps) {
  const { map_center, map_origin } = props;

  const page_base: IVec = TO_XY(L3_BASE(map_center.i));
  const page_center: IVec = TO_XY(L3_CENTER(map_center.i));

  return (
    <group name="test-layer">
      <Dot i={map_center.i} f={map_center.f} color="red" />

      <Dot i={map_origin} color="blue" />

      <GroundedNode i={page_base}>
        <Block base={[0, U_PER_CELL, 0]} span={[1, 1, 1]} color="green" />
      </GroundedNode>
      <GroundedNode i={page_center}>
        <Block base={[U_PER_CELL, 0, 0]} span={[1, 1, 1]} color="red" />
      </GroundedNode>

      <AnchorNode i={map_center.i} f={map_center.f}>
        <Block base={[0, 0, 0]} span={[1, 1, 1]} color="blue" />
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
      {/* 1. [Environment] 지형 및 지리 정보 레이어 */}
      <AnchorSpace map_origin={map_origin}>
        <group name="env-layer">
          <TerrainLayer />
          {/* <POILayer /> */}
        </group>

        {/* 2. [Content] 배치된 복셀 및 오브젝트 */}
        <group name="content-layer">{children}</group>

        {/* 3. [Interaction] 사용자 인터랙션 및 가이드 레이어 */}
        <group name="interaction-layer">
          <CellHighlighter map_center={map_center} map_origin={map_origin} />
          <UserMarker device_coord={device_coord} />
        </group>

        {/* 4. [Debug] 테스트 레이어 */}
        {__IS_DEV__ && <TestNode map_center={map_center} map_origin={map_origin} />}
      </AnchorSpace>
    </group>
  );
});
World.displayName = "World";
