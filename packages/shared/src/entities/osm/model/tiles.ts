/**
 * @file tiles.ts
 * 타일의 물리적 형상(Terrain)과 부가 정보(POIs) 규격을 정의합니다.
 */

import type { TileCoord } from '../lib/utils/tile-utils';
import type { MapPoi } from './osm-feature';

/**
 * 🎯 [SSOT] 채널 키 정의 (바이너리 패킹 및 텍스처 팩토리용)
 */
export const TILE_CHANNEL_KEYS: (keyof TileChannels)[] = [
  'water', // 1. 배경 및 수계 (가장 아래)
  'base', // 2. 지형
  'transport', // 3. 도로망
  'structure', // 4. 건물 (가장 위)
];

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
 * 타일의 기하학적 형상과 속성을 정의하는 핵심 격자 데이터입니다.
 * 평면 속성(Channels)과 수직 정보(Elevation)를 통합 관리합니다.
 */
export interface TileTerrain {
  /** * [Splatmaps/Channels] 지형의 '피부'
   * - 8-bit 속성 데이터. 렌더링 시 어떤 텍스처를 섞을지 결정.
   */
  channels: TileChannels;

  /** * [Elevation/Heightmap] 지형의 '뼈대'
   * - 16-bit 고도 데이터. 메쉬의 수직 위치를 결정.
   */
  elevation: Uint16Array;
}

/**
 * [Tile]
 * 타일 격자 시스템의 원자적 데이터 단위입니다.
 * 모든 공간 데이터는 서버 및 글로벌 기준인 '절대 미터 좌표계'를 유지합니다.
 */
export interface Tile {
  key: string; // "tx:tz" 형식의 고유 키
  coord: TileCoord;

  /** 🏔️ [Terrain] 타일의 형상 그 자체 (Grids 통합) */
  terrain: TileTerrain;

  /** 📍 [POIs] 타일 영역 내의 절대 좌표 기반 포인트 데이터 */
  pois: MapPoi[];
}
