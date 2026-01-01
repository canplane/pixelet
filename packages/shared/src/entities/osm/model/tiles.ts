/**
 * @file tiles.ts
 * 타일 격자 데이터(Grids)와 최종 타일 객체(Tile) 규격을 정의합니다.
 */

import type { TileCoord } from '../lib/utils/tile-utils';
import type { MapPoi } from './osm-feature';

/**
 * 🎯 [SSOT] TileChannels의 모든 키를 순서대로 정의합니다.
 * 이 배열의 순서가 바이너리 패킹(Packer)과 초기화(Factory)의 기준이 됩니다.
 */
export const TILE_CHANNEL_KEYS: (keyof TileChannels)[] = [
  'water', // 1. 배경 및 수계 (가장 아래)
  'base', // 2. 지형
  'transport', // 3. 도로망
  'structure', // 4. 건물 (가장 위)
];

export const TILE_TERRAIN_KEYS: (keyof TileTerrain)[] = ['elevation'];

/**
 * [TileChannels]
 * 수평적 분류를 나타내는 8-bit 데이터 채널 집합입니다.
 * 텍스처 채널(Texture Channels)과 1:1로 대응됩니다.
 */
export interface TileChannels {
  /** [Ch 1] 수계 (Water) - 바다 및 내륙 수계 통합 */
  water: Uint8Array;
  /** [Ch 2] 지형 (Base Land, Natural, Landuse) */
  base: Uint8Array;
  /** [Ch 3] 교통 (Transport) */
  transport: Uint8Array;
  /** [Ch 4] 구조물 (Structure) */
  structure: Uint8Array;
}

/**
 * [TileTerrain]
 * 수직적 형상을 나타내는 16-bit 데이터 집합입니다.
 * DEM(고도) 데이터를 가공한 결과물입니다.
 */
export interface TileTerrain {
  /**
   * [elevation]
   * 각 셀의 복셀 층수 인덱스 (Uint8)
   * 실제 높이(m) = value * (OSM_CONSTANTS.CELL_SIZE * OSM_CONSTANTS.Z_SCALE_RATIO)
   */
  elevation: Uint16Array;
}

/**
 * [TileGrids]
 * OSM 쿼리의 4단계 패스(Pass 1~4)와 정확히 1:1 대응되는 멀티 채널 그리드입니다.
 */
export interface TileGrids {
  /**
   * [Channels] 수평적 분류 데이터 (8-bit)
   * 각 셀이 무엇(물, 도로, 건물 등)인지를 정의합니다.
   */
  channels: TileChannels;

  /**
   * [Terrain] 수직적 형상 데이터 (16-bit)
   * 지형의 실제 높이(고도) 정보를 정밀하게 담습니다.
   */
  terrain: TileTerrain;
}

/**
 * [Tile]
 * 타일 격자 시스템의 원자적 데이터 단위입니다.
 * 모든 공간 데이터는 서버 및 글로벌 기준인 '절대 미터 좌표계'를 유지합니다.
 */
export interface Tile {
  key: string; // "tx:tz" 형식의 고유 키
  coord: TileCoord;
  grids: TileGrids; // 🎯 명칭 통일: 타일 지형 격자 데이터 (Voxel Type Map)
  pois: MapPoi[]; // 타일에 포함된 절대 좌표 기반 POI
}
