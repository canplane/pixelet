import type { MeterPos } from '@/shared/types/map';

/**
 * [voxel-config]
 * 월드의 모든 복셀 규격과 그리드-월드 간 좌표 변환 로직을 관리합니다.
 * 모든 수치는 2의 배수(Binary-friendly)를 기반으로 최적화되어 있습니다.
 */

type CoordXZ = { x: number; z: number };
type CoordXYZ = { x: number; y: number; z: number };

/**
 * [to-center]
 * 🎯 입력 타입에 따라 4m 그리드 내의 중앙 좌표를 반환하는 내부 함수입니다.
 * 큐브 메쉬가 그리드 칸 안에 정중앙에 배치되도록 오프셋(2.0m)을 보정합니다.
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

export const VOXEL_CONFIG = {
  SIZE: 4.0,
  HALF: 2.0,
  QUARTER: 1.0,
  STEP: 0.5, // (미래를 위해 남겨두되, 현재는 4.0 위주로 사용)
  SYSTEM_OFFSET: 0.25,

  toCenter,

  /**
   * [snap-to-voxel]
   * 🎯 물리 좌표를 4m 복셀 단위로 스냅합니다.
   * Floating Origin 리셋 시 이 단위를 기준으로 이동해야 배경 격자가 튀지 않습니다.
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
   * [grid-to-world]
   * 🚀 정수 인덱스를 월드 좌표(큐브의 정중앙)로 변환합니다.
   * 예: (14, 0, 200) -> 실제 3D 공간의 중심점 좌표
   * 큐브의 중심이 y축으로도 절반 올라와야 지면에 딱 붙는 물리적 특성을 반영합니다.
   */
  gridToWorld: (gx: number, gy: number, gz: number) => {
    const S = 4.0;
    const H = 2.0;
    return {
      x: gx * S + H,
      y: gy * S + H,
      z: gz * S + H,
    };
  },
} as const;
