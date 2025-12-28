// 개별 블록 (개발/테스트용)
// 라운딩 블록이 아니라 일반 블록

import { toThreePos } from '@/shared/lib/three/three-coords';
import { VOXEL_CONFIG } from '@pixelet/shared/entities/voxel/voxel-config';

const size = VOXEL_CONFIG.SIZE;
interface VoxelBlockProps {
  /**
   * [grid-pos-standard]
   * gx: East-West (동서)
   * gy: North-South (남북) 🎯
   * gz: Altitude (고도) 🎯
   */
  gridPos: [number, number, number];
  color?: string;
}

export const VoxelBlock = ({ gridPos, color = 'red' }: VoxelBlockProps) => {
  const [gx, gy, gz] = gridPos;

  /**
   * 1. [grid-to-geographic-meter]
   * 정수 격자 좌표를 지리적 미터 좌표(X, Y, Z)로 변환합니다.
   * VOXEL_CONFIG.gridToWorld가 {x, y, z} (y=North)를 반환한다고 가정합니다.
   */
  const geoPos = VOXEL_CONFIG.gridToWorld(gx, gy, gz);

  return (
    <mesh
      /**
       * 2. [geographic-to-three-swizzle]
       * 지리적 좌표를 Three.js 씬 좌표계로 최종 매핑합니다.
       * - World X = geoPos.x (동서)
       * - World Y = geoPos.z (고도) 🎯
       * - World Z = -geoPos.y (북향 반영) 🎯
       */
      position={toThreePos(geoPos)}
    >
      <boxGeometry args={[size, size, size]} />
      <meshStandardMaterial color={color} />
    </mesh>
  );
};
