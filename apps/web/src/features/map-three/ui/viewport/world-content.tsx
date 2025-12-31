import { forwardRef, type ReactNode } from 'react';
import * as THREE from 'three';
import { UserMarker } from '../markers/user-marker';
import { CellHighlighter } from '../markers/cell-highlighter';
import { WorldLayer } from '@/entities/osm/ui/layers/world-layer';
import { useMapNavigation } from '@/entities/osm/model/use-map-navigation';
import { MAP_LAYOUT_THEMES } from '../../model/world-themes';
import { VoxelBlock } from '../voxels/voxel-block';
import { WorldGrid } from '../markers/world-grid';
import { toThreeCoordWithOffset } from '@/shared/lib/three/three-coords';

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
        <WorldLayer />

        {/* 2. [guide-layer] 가이드 레이어 (그리드 및 셀 하이라이터) */}
        {/* [grid-system] Voxel 단위에 최적화된 커스텀 평면 그리드를 사용합니다. */}
        <WorldGrid />

        {/* [prevent-z-fighting] 🎯 지형(0)보다 아주 살짝 높게 배치하여 깜빡임을 방지하는 하이라이터입니다. */}
        <CellHighlighter />

        {/* 3. [dynamic-objects] 동적 오브젝트 (사용자 마커, 복셀 블록, 설치물) */}
        <UserMarker />

        {/* 🎯 [voxel-test-block] 테스트용 복셀 배치 교정
            기존: [4, 0, 0] (X=4, Y=0, Z=0)
            수정: [4, 4, 0] 등으로 테스트 시, 이제 두 번째 인자가 '북쪽'으로 4칸 이동함을 의미합니다.
            만약 VoxelBlock 내부가 [x, y, z] 규격(y=Up)이라면, 
            여기서 넘겨줄 때 [x, z, -y] 형태로 변환해서 그리도록 내부 로직도 점검이 필요합니다.
        */}
        <VoxelBlock gridPos={[4, 4, 0]} color={targetTheme.grid.section} />

        {/* 4. [extension-slot] 외부에서 주입된 추가 레이어나 오브젝트 */}
        {children}

        {/* [debug-origin-marker]
            Floating Origin 리셋 시 이 마커는 항상 Scene의 (0,0,0)에 위치합니다.
            지면(Y=0)보다 살짝 위(0.05)에 배치되어 기준점 역할을 합니다.
        */}
        <mesh position={toThreeCoordWithOffset({ x: 0, y: 0 }, 0.05)}>
          <sphereGeometry args={[0.2]} />
          <meshBasicMaterial color="red" />
        </mesh>
      </group>
    );
  },
);

WorldContent.displayName = 'WorldContent';
