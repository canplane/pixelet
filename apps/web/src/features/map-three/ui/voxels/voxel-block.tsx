// 개별 블록 (개발/테스트용)
// 라운딩 블록이 아니라 일반 블록

import { toThreeCoord } from '@/shared/lib/three/three-coords';
import { VOXEL_SIZE } from '@pixelet/core/entities/voxel/voxel-config';
import { CoordOps, type Coord } from '@pixelet/core/shared/model/coords';

const { BASE, HALF } = VOXEL_SIZE;

interface CellCoord {
  cx: number;
  cy: number;
  cz: number;
}

interface VoxelBlockProps {
  /**
   * cx: East-West (동서)
   * cy: North-South (남북) 🎯
   * cz: Altitude (고도) 🎯
   */
  cellCoord: CellCoord;
  color?: string;
}

const cellToWorld = (cellCoord: CellCoord): Coord => {
  const { cx, cy, cz } = cellCoord;
  return CoordOps.add({ x: cx, y: cy, z: cz }, HALF);
};

export const VoxelBlock = ({ cellCoord, color = 'red' }: VoxelBlockProps) => {
  const pos = cellToWorld(cellCoord);

  return (
    <mesh position={toThreeCoord(pos)}>
      <boxGeometry args={[BASE, BASE, BASE]} />
      <meshStandardMaterial color={color} />
    </mesh>
  );
};
