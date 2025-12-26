import { forwardRef, type ReactNode } from 'react';
import * as THREE from 'three';
import { UserMarker } from '../markers/user-marker';
import { CellHighlighter } from '../markers/cell-highlighter';
import { OsmLayer } from '../layers/osm-layer';
import { useMapNavigation } from '../../model/use-map-navigation';
import { MAP_LAYOUT_THEMES } from '../../model/world-themes';
import { VoxelBlock } from '../voxels/voxel-block.box';
import { VoxelGridPlanar } from '../markers/voxel-grid-planar';

/**
 * [world-content]
 * 지도의 모든 시각적 레이어(지형, 그리드, 마커, 오브젝트)를 결합하는 최상위 컨테이너입니다.
 * WorldGroupRef를 통해 Floating Origin 기반의 위치 전역 이동을 수행합니다.
 */

interface WorldContentProps {
  children?: ReactNode;
}

export const WorldContent = forwardRef<THREE.Group, WorldContentProps>(
  ({ children }, ref) => {
    const { themeKey } = useMapNavigation();
    const targetTheme = MAP_LAYOUT_THEMES[themeKey];

    return (
      <group ref={ref}>
        {/* 1. [base-layer] 기본 레이어 (OSM 지형 및 바닥 메쉬) */}
        <OsmLayer />

        {/* 2. [guide-layer] 가이드 레이어 (그리드 및 셀 하이라이터) */}
        {/* [grid-system] Voxel 단위에 최적화된 커스텀 평면 그리드를 사용합니다. */}
        <VoxelGridPlanar />

        {/* [prevent-z-fighting] 🎯 지형(0)보다 아주 살짝 높게 배치하여 깜빡임을 방지하는 하이라이터입니다. */}
        <CellHighlighter />

        {/* 3. [dynamic-objects] 동적 오브젝트 (사용자 마커, 복셀 블록, 설치물) */}
        <UserMarker />

        {/* [voxel-test-block] 특정 그리드 위치에 배치된 테스트용 복셀입니다. */}
        <VoxelBlock gridPos={[4, 0, 0]} color={targetTheme.grid.section} />

        {/* 4. [extension-slot] 외부에서 주입된 추가 레이어나 오브젝트 */}
        {children}

        {/* [debug-origin-marker] 🎯 
            디버그용 기준점 마커: Floating Origin 리셋 시 이 마커가 원점(0,0,0)에 위치해야 합니다. 
        */}
        <mesh position={[0, 0.05, 0]}>
          <sphereGeometry args={[0.2]} />
          <meshBasicMaterial color="red" />
        </mesh>
      </group>
    );
  },
);

WorldContent.displayName = 'WorldContent';
