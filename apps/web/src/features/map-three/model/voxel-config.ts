import type { MeterPos } from '@/shared/types/map';

type CoordXZ = { x: number; z: number };
type CoordXYZ = { x: number; y: number; z: number };

/**
 * 🎯 입력 타입에 따라 4m 그리드 내의 중앙 좌표를 반환하는 내부 함수
 */
function toCenter(pos: number): number;
function toCenter(pos: CoordXZ): CoordXZ;
function toCenter(pos: CoordXYZ): CoordXYZ;
function toCenter(input: any): any {
  const H = 2.0; // VOXEL_CONFIG.HALF
  if (typeof input === 'number') return input + H;
  if ('y' in input) return { x: input.x + H, y: input.y + H, z: input.z + H };
  return { x: input.x + H, z: input.z + H };
}

/**
 * 🚀 월드의 모든 복셀 규격을 관리하는 핵심 설정
 */
export const VOXEL_CONFIG = {
  SIZE: 4.0,
  HALF: 2.0,
  QUARTER: 1.0,
  STEP: 0.5, // (미래를 위해 남겨두되, 현재는 4.0 위주로 사용)
  SYSTEM_OFFSET: 0.25,

  toCenter,

  /**
   * 🎯 물리 좌표를 4m 복셀 단위로 스냅합니다.
   */
  snapToVoxel(pos: MeterPos) {
    const S = 4.0;
    return {
      x: Math.floor(pos.x / S) * S,
      y: Math.floor(pos.y / S) * S,
      z: Math.floor(pos.z / S) * S,
    };
  },

  /**
   * 🚀 정수 인덱스를 월드 좌표(큐브의 정중앙)로 변환
   * 예: (14, 0, 200) -> 실제 3D 공간의 중심점 좌표
   */
  gridToWorld: (gx: number, gy: number, gz: number) => {
    const S = 4.0;
    const H = 2.0;
    return {
      x: gx * S + H,
      y: gy * S + H, // 큐브의 중심이 y축으로도 절반 올라와야 지면에 딱 붙음
      z: gz * S + H,
    };
  },
} as const;
