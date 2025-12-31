import {
  CoordOps,
  type Coord,
  type Coord2D,
} from '@pixelet/shared/shared/model/coords';

/**
 * [voxel-config]
 * 월드의 모든 복셀 규격과 그리드-월드 간 좌표 변환 로직을 관리합니다.
 * 모든 수치는 2의 배수(Binary-friendly)를 기반으로 최적화되어 있습니다.
 */

const SIZE = 4.0;
const HALF = 2.0;
const QUARTER = 1.0;
const STEP = 0.5; // (미래를 위해 남겨두되, 현재는 4.0 위주로 사용)
const SYSTEM_OFFSET = 0.25;

/**
 * [to-center]
 * 🎯 CoordOps.add를 활용하여 4m 그리드 내의 중앙 좌표로 보정합니다.
 * (x, y, z) 각 성분에 HALF(2.0m)를 더하는 연산을 캡슐화합니다.
 */
export const toCenter = <T extends number | Coord2D | Coord>(input: T): T => {
  const H = VOXEL_CONFIG.HALF;

  // 1. 단순 숫자인 경우
  if (typeof input === 'number') {
    return (input + H) as T;
  }

  // 2. 좌표 객체인 경우 (Coord2D | Coord)
  // CoordOps.add가 제네릭 T를 반환하므로 타입 캐스팅 없이 안전하게 환원됩니다.
  return CoordOps.add(input as Coord2D | Coord, H) as T;
};

export const VOXEL_CONFIG = {
  SIZE,
  HALF,
  QUARTER,
  STEP,
  SYSTEM_OFFSET,

  toCenter,

  /**
   * [snap-to-voxel]
   * 🎯 물리 좌표를 4m 복셀 단위로 스냅합니다.
   * Floating Origin 리셋 시 이 단위를 기준으로 이동해야 배경 격자가 튀지 않습니다.
   * 💡 이제 입력된 pos.y는 북향(North) 좌표입니다.
   */
  snapToVoxel(pos: Coord): Coord {
    const S = 4.0;
    return {
      x: Math.floor(pos.x / S) * S,
      y: Math.floor(pos.y / S) * S, // 북향 스냅
      z: Math.floor(pos.z / S) * S, // 고도 스냅
    };
  },

  /**
   * [grid-to-world]
   * 🚀 정수 인덱스(그리드 번호)를 월드 미터 좌표(큐브의 정중앙)로 변환합니다.
   * 💡 결과: { x, y, z } (y는 북향 미터, z는 고도 미터)
   */
  gridToWorld: (gx: number, gy: number, gz: number): Coord => {
    const S = 4.0;
    const H = 2.0;
    return {
      x: gx * S + H,
      y: gy * S + H, // 남북 방향 중심점
      z: gz * S + H, // 고도 방향 중심점
    };
  },
} as const;
