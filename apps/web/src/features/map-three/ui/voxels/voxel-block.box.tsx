// 개별 블록 (개발/테스트용)
// 라운딩 블록이 아니라 일반 블록

import { VOXEL_CONFIG } from '@pixelet/shared/entities/voxel/voxel-config';

const size = VOXEL_CONFIG.SIZE;
interface VoxelBlockProps {
  gridPos: [number, number, number]; // [gx, gy, gz] 정수 좌표
  color?: string;
}

export const VoxelBlock = ({ gridPos, color = 'red' }: VoxelBlockProps) => {
  // 정수 좌표를 월드 좌표로 변환 후 Floating Origin 차감
  const localPos = VOXEL_CONFIG.gridToWorld(...gridPos);

  return (
    <mesh position={[localPos.x, localPos.y, localPos.z]}>
      <boxGeometry args={[size, size, size]} />
      <meshStandardMaterial color={color} />
    </mesh>
  );
};
