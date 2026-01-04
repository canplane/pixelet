import { GEO_CONSTANTS } from '@pixelet/core/entities/geo/model/geo-constants';

const { CELL_SIZE } = GEO_CONSTANTS;

/**
 * [voxel-config]
 * 월드의 모든 복셀 규격과 그리드-월드 간 좌표 변환 로직을 관리합니다.
 * 모든 수치는 2의 배수(Binary-friendly)를 기반으로 최적화되어 있습니다.
 */

const BASE = CELL_SIZE;
const HALF = CELL_SIZE / 2;
const QUARTER = CELL_SIZE / 4;
const STEP = CELL_SIZE / 8; // (미래를 위해 남겨두되, 현재는 4.0 위주로 사용)
const SYSTEM_OFFSET = CELL_SIZE / 16;

export const VOXEL_SIZE = {
  BASE,
  HALF,
  QUARTER,
  STEP,
  SYSTEM_OFFSET,
} as const;
