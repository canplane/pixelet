/**
 * @file tiles.ts
 * 타일 격자 데이터(Grids)와 최종 타일 객체(GeometryTile) 규격을 정의합니다.
 */

import type { Coord } from '@pixelet/shared/shared/model/coords';
import type { MapPoi } from './osm-feature';
import type { TileCoord } from '../lib/utils/tile-utils';

/**
 * [GridLayers]
 * 수평적 분류를 나타내는 8-bit 레이어 집합입니다.
 * OSM 데이터를 래스터화한 결과물입니다.
 */
export interface GridLayers {
  base: Uint8Array; // [Pass 1] 지형 (Landuse, Natural, Institutional)
  water: Uint8Array; // [Pass 2] 수계 (Water)
  transport: Uint8Array; // [Pass 3] 교통 (Road, Rail, Pedestrian, Parking)
  structure: Uint8Array; // [Pass 4] 구조물 (Building, Man-made)
}

/**
 * [GridTerrain]
 * 수직적 형상을 나타내는 16-bit 데이터 집합입니다.
 * DEM(고도) 데이터를 가공한 결과물입니다.
 */
export interface GridTerrain {
  /**
   * [height]
   * 각 셀의 복셀 층수 인덱스 (Uint8)
   * 실제 높이(m) = value * (OSM_CONSTANTS.CELL_SIZE * OSM_CONSTANTS.Z_SCALE_RATIO)
   */
  height: Uint16Array;
}

/**
 * [TileGrids]
 * OSM 쿼리의 4단계 패스(Pass 1~4)와 정확히 1:1 대응되는 멀티 채널 그리드입니다.
 */
export interface TileGrids {
  /**
   * [Layers] 수평적 분류 데이터 (8-bit)
   * 각 셀이 무엇(물, 도로, 건물 등)인지를 정의합니다.
   */
  layers: GridLayers;

  /**
   * [Terrain] 수직적 형상 데이터 (16-bit)
   * 지형의 실제 높이(고도) 정보를 정밀하게 담습니다.
   */
  terrain: GridTerrain;
}

/**
 * [GeometryTile]
 * 타일 격자 시스템의 원자적 데이터 단위입니다.
 * 모든 공간 데이터는 서버 및 글로벌 기준인 '절대 미터 좌표계'를 유지합니다.
 */
export interface GeometryTile {
  coord: TileCoord;
  // key: string; // "tx:tz" 형식의 고유 키
  // origin: Coord; // 타일의 시작점 (절대 미터 좌표)
  grids: TileGrids; // 🎯 명칭 통일: 타일 지형 격자 데이터 (Voxel Type Map)
  pois: MapPoi[]; // 타일에 포함된 절대 좌표 기반 POI
  // resolution: number; // 격자 해상도
}
