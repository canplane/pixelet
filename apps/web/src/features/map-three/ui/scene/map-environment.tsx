import { Grid } from '@react-three/drei';
import { useMapStore } from '../../model/map-store';
import { projection } from '@/shared/lib/geo/projection';

export function MapEnvironment() {
  const center = useMapStore((state) => state.center);
  const gridSize = projection.GRID_SIZE;

  return (
    <>
      {/* 1. 안개 (Fog): 멀리 있는 물체를 자연스럽게 가림 */}
      <fog attach="fog" args={['#111', 100, 5000]} />

      {/* 2. 조명 (Lighting) */}
      <ambientLight intensity={1.5} />
      <directionalLight
        // 광원이 현재 지도의 중심 상공에 위치하도록 설정
        position={[center?.x || 0, 1000, center?.z || 0]}
        intensity={1}
        castShadow // 필요 시 활성화
      />

      {/* 3. 무한 그리드 (Infinite Grid) */}
      <Grid
        infiniteGrid
        followCamera
        fadeDistance={5000}
        cellSize={gridSize}
        sectionSize={100}
        sectionColor="#444444"
        cellColor="#222222"
        // 격자 중심 보정
        position={[gridSize / 2, -0.01, gridSize / 2]}
      />
    </>
  );
}
