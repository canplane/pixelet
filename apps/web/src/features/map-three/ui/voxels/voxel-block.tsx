// 개별 블록 (개발/테스트용)
// 라운딩 블록이 아니라 일반 블록

import { toThreeCoord } from '@/shared/lib/three/three-coords';
import { VOXEL_CONFIG } from '@pixelet/shared/entities/voxel/voxel-config';
import type { Coord } from '@pixelet/shared/shared/model/coords';

const { SIZE: size } = VOXEL_CONFIG;
interface VoxelBlockProps {
  /**
   * cx: East-West (동서)
   * cy: North-South (남북) 🎯
   * cz: Altitude (고도) 🎯
   */
  cellIdx: { cx: number; cy: number; cz: number };
  color?: string;
}

/**
 * [grid-to-world]
 * 🚀 정수 인덱스(그리드 번호)를 월드 미터 좌표(큐브의 정중앙)로 변환합니다.
 * 💡 결과: { x, y, z } (y는 북향 미터, z는 고도 미터)
 */
const gridToWorld = (gx: number, gy: number, gz: number): Coord => {
  const S = 4.0;
  const H = 2.0;
  return {
    x: gx * S + H,
    y: gy * S + H, // 남북 방향 중심점
    z: gz * S + H, // 고도 방향 중심점
  };
};

export const VoxelBlock = ({ cellIdx, color = 'red' }: VoxelBlockProps) => {
  const { cx, cy, cz } = cellIdx;

  /**
   * 1. [grid-to-geographic-meter]
   * 정수 격자 좌표를 지리적 미터 좌표(X, Y, Z)로 변환합니다.
   * VOXEL_CONFIG.gridToWorld가 {x, y, z} (y=North)를 반환한다고 가정합니다.
   */
  const geoPos = gridToWorld(cx, cy, cz);

  return (
    <mesh
      /**
       * 2. [geographic-to-three-swizzle]
       * 지리적 좌표를 Three.js 씬 좌표계로 최종 매핑합니다.
       * - World X = geoPos.x (동서)
       * - World Y = geoPos.z (고도) 🎯
       * - World Z = -geoPos.y (북향 반영) 🎯
       */
      position={toThreeCoord(geoPos)}
    >
      <boxGeometry args={[size, size, size]} />
      <meshStandardMaterial color={color} />
    </mesh>
  );
};
