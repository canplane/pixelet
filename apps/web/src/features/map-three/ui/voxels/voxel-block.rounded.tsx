// 개별 블록 (개발/테스트용)

import { VOXEL_CONFIG } from '@pixelet/shared/entities/voxel/voxel-config';
import { RoundedBox } from '@react-three/drei';

const size = VOXEL_CONFIG.SIZE;

interface VoxelBlockProps {
  gridPos: [number, number, number];
  color?: string;
  radius?: number; // 모서리 둥글기 정도
}

export const VoxelBlock = ({
  gridPos,
  color = '#FF6B6B',
  radius = 0.5, // 4m 큐브 기준 0.5m 정도면 충분히 부드럽습니다.
}: VoxelBlockProps) => {
  // 정수 좌표를 월드 좌표로 변환 후 Floating Origin 차감
  const localPos = VOXEL_CONFIG.gridToWorld(...gridPos);

  return (
    <RoundedBox
      args={[size, size, size]} // Width, Height, Depth
      radius={radius} // 모서리 반지름
      smoothness={4} // 세그먼트 수 (너무 높으면 성능 저하, 4 정도가 적당)
      position={[localPos.x, localPos.y, localPos.z]}
      creaseAngle={0.4} // 🚀 면과 면 사이의 경계를 부드럽게 처리하는 각도
    >
      <meshStandardMaterial
        color={color}
        roughness={0.7} // 약간 매트한 느낌이 Cozy함에 도움됩니다.
        metalness={0.1}
      />
    </RoundedBox>
  );
};
