import { forwardRef, type ReactNode } from 'react';
import * as THREE from 'three';

import { useMapNavigation } from '@/entities/osm/model/use-map-navigation';
import { MAP_LAYOUT_THEMES } from '../../model/world-themes';

import { TerrainLayer } from '@/entities/osm/ui/layers/terrain-layer';
import { PoiLayer } from '@/entities/osm/ui/layers/poi-layer';
import { UserMarker } from '../markers/user-marker';
import { CellHighlighter } from '../markers/cell-highlighter';
import { VoxelBlock } from '../voxels/voxel-block';
import { toThreeCoordWithOffset } from '@/shared/lib/three/three-coords';
import { TerrainStickyGroup } from '@/features/map-three/ui/terrain-sticky-group';
import { VOXEL_CONFIG } from '@pixelet/shared/entities/voxel/voxel-config';

const { snapToVoxel } = VOXEL_CONFIG;

/**
 * [WorldContent]
 * 지도의 모든 시각적 레이어(지형, 마커, 오브젝트)를 통합하는 최상위 컨테이너입니다.
 * * * 🏗️ [Structure]
 * 1. Environment: 지형, POI (Base Map)
 * 2. Content: 복셀, 설치물 (User Content)
 * 3. Interaction: 하이라이터, 마커 (UI/UX)
 * * * 🌐 [Coordinate System]
 * 이 그룹 내의 모든 자식 요소는 Floating Origin 시스템에 의해
 * 자신의 로컬 좌표를 스스로 보정(Self-Correction)해야 합니다.
 */

interface WorldContentProps {
  children?: ReactNode;
}

export const WorldContent = forwardRef<THREE.Group, WorldContentProps>(
  ({ children }, ref) => {
    const { center, floatingOrigin, themeKey } = useMapNavigation();
    const targetTheme = MAP_LAYOUT_THEMES[themeKey];

    return (
      <group ref={ref} name="world-content">
        {/* 1. [Environment] 지형 및 지리 정보 레이어 */}
        <group name="env-layer">
          <TerrainLayer />
          <PoiLayer />
        </group>

        {/* 2. [Content] 배치된 복셀 및 오브젝트 */}
        <group name="content-layer">
          {/* 테스트용 복셀 (북쪽 4칸 이동 배치 예시) */}
          {/* <TerrainStickyGroup coord={snapToVoxel(center!)}>
            <VoxelBlock
              cellIdx={{ cx: 4, cy: 4, cz: 0 }}
              color={targetTheme.grid.section}
            />
          </TerrainStickyGroup>
          {children} */}
        </group>

        {/* 3. [Interaction] 사용자 인터랙션 및 가이드 레이어 */}
        <group name="interaction-layer">
          {/* Z-Fighting 방지를 위해 지형보다 미세하게 높게(offset) 렌더링 */}
          <CellHighlighter />
          <UserMarker />

          {/* <WorldGrid /> : 추후 가이드 필요 시 활성화 */}
        </group>

        {/* 4. [Debug] 원점 확인용 마커 (항상 0,0,0) */}
        {/* {import.meta.env.DEV && (
          <TerrainStickyGroup coord={snapToVoxel(center!)}>
            <mesh
              name="debug-origin"
              position={toThreeCoordWithOffset({ x: 0, y: 0 }, 0.05)}
            >
              <sphereGeometry args={[0.2]} />
              <meshBasicMaterial color="red" wireframe />
            </mesh>
          </TerrainStickyGroup>
        )} */}
      </group>
    );
  },
);

WorldContent.displayName = 'WorldContent';
