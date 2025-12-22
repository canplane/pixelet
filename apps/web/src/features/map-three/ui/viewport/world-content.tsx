import { forwardRef } from 'react';
import * as THREE from 'three';
import { Grid } from '@react-three/drei';
import { UserMarker } from '../markers/user-marker';
import { CellHighlighter } from '../markers/cell-highlighter';
import { OsmLayer } from '../osm/osm-layer';
import { projection } from '@/shared/lib/geo/projection';
import { THREE_CONFIG } from '../../model/three-config';

const { GRID_SIZE } = projection;
const { FOG_FAR } = THREE_CONFIG.RENDERING;

export const WorldContent = forwardRef<
  THREE.Group,
  { children?: React.ReactNode }
>(({ children }, ref) => {
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

      {/* 🚀 그리드를 땅(OSM 타일)과 함께 배치 */}
      {/* <Grid
        args={[FOG_FAR * 3, FOG_FAR * 3]} // 안개 거리보다 충분히 크게 설정
        infiniteGrid={true} // 🚀 쉐이더 기반 안티앨리어싱 활성화
        followCamera={false} // 🚀 위치는 worldGroup에 고정 (이중 이동 방지)
        // 🚀 [핵심 수정]
        // 1. Frustum Culling을 끕니다. (Drei Grid는 내부 mesh에 이 속성을 전달해야 함)
        frustumCulled={false}
        // 2. 렌더링 순서를 뒤로 밀어 안개나 다른 오브젝트와 겹칠 때 안정적으로 출력
        renderOrder={-1}
        fadeDistance={FOG_FAR}
        fadeStrength={5}
        cellSize={GRID_SIZE}
        sectionSize={GRID_SIZE * 10} // 10칸마다 굵은 선
        // sectionColor={targetTheme.grid.section}
        // cellColor={targetTheme.grid.cell}
        // 🚀 색상을 테마보다 약간 더 흐릿하게(Muted) 가져가면 지직거림이 눈에 훨씬 덜 띕니다.
        sectionColor="#2d5a27"
        cellColor="#3e7a37"
        cellThickness={0.8}
        sectionThickness={1.5}
        position={[0, -0.01, 0]}
      /> */}

      {/* 테스트용 빨간 사각형 (실제 100m x 100m) */}
      {/* {center && (
        <mesh
          rotation={[-Math.PI / 2, 0, 0]}
          position={[center.x, center.y, center.z]}
        >
          <planeGeometry args={[100, 100]} />
          <meshBasicMaterial color="red" wireframe />
        </mesh>
      )} */}

      {/* 추후 추가될 모든 지형물(건물, 도로, Voxel)은 
         여기에 배치되어야 카메라 리셋 시 함께 이동합니다.
      */}
    </group>
  );
});

WorldContent.displayName = 'WorldContent';
