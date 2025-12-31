/**
 * @file tiles.ts
 * 타일 격자 데이터(Grids)와 최종 타일 객체(GeometryTile) 규격을 정의합니다.
 */

import type { Coord } from '@pixelet/shared/shared/model/coords';
import type { MapInfra, MapPoi } from './osm-feature';

/**
 * [VoxelLayers]
 * 수평적 분류를 나타내는 8-bit 레이어 집합입니다.
 * OSM 데이터를 래스터화한 결과물입니다.
 */
export interface VoxelLayers {
  base: Uint8Array; // [Pass 1] 지형 (Landuse, Natural, Institutional)
  water: Uint8Array; // [Pass 2] 수계 (Water)
  transport: Uint8Array; // [Pass 3] 교통 (Road, Rail, Pedestrian, Parking)
  structure: Uint8Array; // [Pass 4] 구조물 (Building, Man-made)
}

/**
 * [VoxelTerrain]
 * 수직적 형상을 나타내는 16-bit 데이터 집합입니다.
 * DEM(고도) 데이터를 가공한 결과물입니다.
 */
export interface VoxelTerrain {
  /**
   * [height]
   * 각 셀의 복셀 층수 인덱스 (Uint8)
   * 실제 높이(m) = value * (OSM_CONSTANTS.CELL_SIZE * OSM_CONSTANTS.Z_SCALE_RATIO)
   */
  height: Uint16Array;
}

/**
 * [GeometryGrids]
 * OSM 쿼리의 4단계 패스(Pass 1~4)와 정확히 1:1 대응되는 멀티 채널 그리드입니다.
 */
export interface GeometryGrids {
  /**
   * [Layers] 수평적 분류 데이터 (8-bit)
   * 각 셀이 무엇(물, 도로, 건물 등)인지를 정의합니다.
   */
  layers: VoxelLayers;

  /**
   * [Terrain] 수직적 형상 데이터 (16-bit)
   * 지형의 실제 높이(고도) 정보를 정밀하게 담습니다.
   */
  terrain: VoxelTerrain;
}

/**
 * [GeometryTile]
 * 타일 격자 시스템의 원자적 데이터 단위입니다.
 * 모든 공간 데이터는 서버 및 글로벌 기준인 '절대 미터 좌표계'를 유지합니다.
 */
export interface GeometryTile {
  key: string; // "tx:tz" 형식의 고유 키
  origin: Coord; // 타일의 시작점 (절대 미터 좌표)
  grids: GeometryGrids; // 🎯 명칭 통일: 타일 지형 격자 데이터 (Voxel Type Map)
  pois: MapPoi[]; // 타일에 포함된 절대 좌표 기반 POI
  infras: MapInfra[]; // 타일에 포함된 절대 좌표 기반 인프라
  resolution: number; // 격자 해상도
}

/**
 * [VoxelLayers 초기화 팩토리]
 */
export function createEmptyLayers(resolution: number): VoxelLayers {
  const size = resolution * resolution;
  return {
    base: new Uint8Array(size),
    water: new Uint8Array(size),
    transport: new Uint8Array(size),
    structure: new Uint8Array(size),
  };
}

/**
 * [Factory] 빈 그리드 생성기
 * 모든 채널을 동일한 해상도로 생성하며, 고도(height) 채널은 0층으로 초기화합니다.
 */
export function createEmptyGrids(resolution: number): GeometryGrids {
  const size = resolution * resolution;
  return {
    layers: createEmptyLayers(resolution),
    terrain: {
      // 🎯 고도 채널 추가: 기본값 0 (해수면 또는 기준 고도)
      height: new Uint16Array(size).fill(0),
    },
  };
}
