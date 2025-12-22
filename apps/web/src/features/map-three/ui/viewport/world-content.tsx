import { forwardRef } from 'react';
import * as THREE from 'three';
import { Grid } from '@react-three/drei';
import { UserMarker } from '../markers/user-marker';
import { CellHighlighter } from '../markers/cell-highlighter';
import { OsmLayer } from '../osm/osm-layer';
import { projection } from '@/shared/lib/geo/projection';
import { THREE_CONFIG } from '../../model/three-config';
import { useMapNavigation } from '../../model/use-map-navigation';
import { MAP_THEMES } from '../../model/map-theme'; // 🚀 테마 데이터 임포트

const { GRID_SIZE, HALF_GRID } = projection;
const { FOG_FAR } = THREE_CONFIG.RENDERING;

export const WorldContent = forwardRef<
  THREE.Group,
  { children?: React.ReactNode }
>(({ children }, ref) => {
  // 1. 스토어에서 현재 시간대에 맞는 테마 키를 가져옵니다 ('morning', 'day' 등)
  const { themeKey } = useMapNavigation();

  // 2. 해당 키로 실제 테마 설정 객체를 추출합니다
  const targetTheme = MAP_THEMES[themeKey];

  // 🚀 모든 3D 지형물은 이 그룹 내부에 위치해야 Floating Origin 보정을 받습니다
  return (
    <group ref={ref}>
      {/* 내 위치 마커 */}
      <UserMarker />

      {/* OSM 데이터 전용 레이어: 내부적으로 fetch와 렌더링 수행 */}
      <OsmLayer />

      <CellHighlighter />

      {/* 확장성을 위한 추가 자식 (커스텀 핀 등) */}
      {children}

      {/* 🚀 [그리드 정렬] 
          모든 Mesh가 정수 좌표(0, 4, 8...)에 배치될 때 
          박스의 정중앙이 오도록 그리드 자체를 HALF_GRID만큼 밀어줍니다. */}
      <Grid
        // 🚀 너무 크면 지평선 끝까지 하얀 선이 뭉쳐 보입니다. 적당히 조절하세요.
        args={[FOG_FAR, FOG_FAR]}
        infiniteGrid={true}
        // 🚀 안개(FOG_FAR)보다 먼저 그리드가 사라지게 해서 하늘과의 경계를 만듭니다.
        fadeDistance={FOG_FAR * 0.8}
        fadeStrength={5}
        cellSize={GRID_SIZE}
        sectionSize={GRID_SIZE * 10}
        sectionColor={targetTheme.grid.section}
        cellColor={targetTheme.grid.cell}
        cellThickness={0.5}
        sectionThickness={1.0}
        // 🚀 위치를 -0.1 이하로 내려서 지형(landuse) 아래로 확실히 보냅니다.
        position={[HALF_GRID, -0.1, HALF_GRID]}
      />

      {/* 추후 추가될 모든 지형물(건물, 도로, Voxel)은 
         여기에 배치되어야 카메라 리셋 시 함께 이동합니다.
      */}

      {/* 🚀 [기준 테스트 박스] 
          이 박스가 정수 좌표(0, 0, 0) 기준에서 HALF_GRID만큼 이동해 있으므로
          그리드 칸 안에 완벽하게 들어맞게 됩니다. */}
      <mesh position={[HALF_GRID, 0.1, HALF_GRID]}>
        <boxGeometry args={[GRID_SIZE, 0.2, GRID_SIZE]} />
        <meshBasicMaterial
          color={targetTheme.grid.section}
          transparent
          opacity={0.4}
        />
      </mesh>
    </group>
  );
});

WorldContent.displayName = 'WorldContent';
