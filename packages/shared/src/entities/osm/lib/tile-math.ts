/**
 * @file tile-math.ts
 * 타일 인덱스와 절대 좌표(Meter), 지리적 좌표(Location) 간의 변환을 담당합니다.
 */

import { projection } from '@pixelet/shared/shared/lib/map/projection';
import { OSM_CONFIG } from '../osm-config';
import type { MeterPos } from '@pixelet/shared/shared/model/coords';

const { unproject } = projection;
const { TILE_SIZE } = OSM_CONFIG;

/**
 * 절대 좌표(MeterPos)를 받아 타일 인덱스(tx, ty)로 변환합니다.
 */
export const getTileIndex = (pos: MeterPos) => {
  return {
    tx: Math.floor(pos.x / TILE_SIZE),
    ty: Math.floor(pos.y / TILE_SIZE),
  };
};

export interface Bbox {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
}

export interface LocationBbox {
  minLat: number;
  minLng: number;
  maxLat: number;
  maxLng: number;
}

/**
 * 특정 타일 인덱스의 미터 단위 BBox를 반환합니다.
 * * @param tx - 타일 X 인덱스
 * @param ty - 타일 Y 인덱스
 */
export const getTileBbox = (tx: number, ty: number): Bbox => {
  const { TILE_SIZE } = OSM_CONFIG;

  const minX = tx * TILE_SIZE;
  const minY = ty * TILE_SIZE;
  const maxX = (tx + 1) * TILE_SIZE;
  const maxY = (ty + 1) * TILE_SIZE;

  return { minX, minY, maxX, maxY };
};

/**
 * [상위 호환] 특정 타일의 지리적 위경도 범위(LocationBbox)를 반환합니다.
 * Overpass API 쿼리나 서버사이드 공간 검색에 직접적으로 사용됩니다.
 */
export const getTileLocationBbox = (tx: number, ty: number): LocationBbox => {
  const { minX, minY, maxX, maxY } = getTileBbox(tx, ty);

  // 미터 좌표를 위경도로 역투영(Unproject)
  const sw = unproject({ x: minX, y: minY, z: 0 });
  const ne = unproject({ x: maxX, y: maxY, z: 0 });

  // Overpass 및 PostGIS 표준 형식 (S, W, N, E) 보장
  return {
    minLat: Math.min(sw.lat, ne.lat),
    minLng: Math.min(sw.lng, ne.lng),
    maxLat: Math.max(sw.lat, ne.lat),
    maxLng: Math.max(sw.lng, ne.lng),
  };
};
