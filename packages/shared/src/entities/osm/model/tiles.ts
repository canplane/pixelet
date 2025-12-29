/**
 * @file tiles.ts
 * 타일 격자 데이터(Grids)와 최종 타일 객체(WorldTile) 규격을 정의합니다.
 */

import type { MeterPos } from '@pixelet/shared/shared/model/coords';
import { VOXEL_TYPES } from '../../voxel/model/voxels';
import type { MapInfra, MapPoi } from './osms';

/**
 * [TileGrids]
 * OSM 쿼리의 4단계 패스(Pass 1~4)와 정확히 1:1 대응되는 멀티 채널 그리드입니다.
 */
export interface TileGrids {
  base: Uint8Array; // [Pass 1] 지형 (Landuse, Natural, Institutional)
  water: Uint8Array; // [Pass 2] 수계 (Water)
  transport: Uint8Array; // [Pass 3] 교통 (Road, Rail, Pedestrian, Parking)
  structure: Uint8Array; // [Pass 4] 구조물 (Building, Man-made)

  /**
   * [height]
   * 각 셀의 복셀 층수 인덱스 (Uint8)
   * 실제 높이(m) = value * (OSM_CONFIG.CELL_SIZE * OSM_CONFIG.Z_SCALE_RATIO)
   */
  height: Uint8Array;
}

/**
 * [Factory] 빈 그리드 생성기
 * 모든 채널을 동일한 해상도로 생성하며, 고도(height) 채널은 0층으로 초기화합니다.
 */
export function createEmptyGrids(resolution: number): TileGrids {
  const size = resolution * resolution;
  return {
    base: new Uint8Array(size).fill(VOXEL_TYPES.BASE_LAND),
    water: new Uint8Array(size).fill(VOXEL_TYPES.UNKNOWN),
    transport: new Uint8Array(size).fill(VOXEL_TYPES.UNKNOWN),
    structure: new Uint8Array(size).fill(VOXEL_TYPES.UNKNOWN),

    // 🎯 고도 채널 추가: 기본값 0 (해수면 또는 기준 고도)
    height: new Uint8Array(size).fill(0),
  };
}

/**
 * [world-tile]
 * 타일 격자 시스템의 원자적 데이터 단위입니다.
 * 모든 공간 데이터는 서버 및 글로벌 기준인 '절대 미터 좌표계'를 유지합니다.
 */
export interface WorldTile {
  key: string; // "tx:tz" 형식의 고유 키
  origin: MeterPos; // 타일의 시작점 (절대 미터 좌표)
  grids: TileGrids; // 🎯 명칭 통일: 타일 지형 격자 데이터 (Voxel Type Map)
  pois: MapPoi[]; // 타일에 포함된 절대 좌표 기반 POI
  infras: MapInfra[]; // 타일에 포함된 절대 좌표 기반 인프라
  resolution: number; // 격자 해상도
}
