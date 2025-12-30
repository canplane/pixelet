/**
 * @file tile-math.ts
 * 타일 인덱스와 절대 좌표(Meter), 지리적 좌표(Location) 간의 변환을 담당합니다.
 */

import { projection } from '@pixelet/shared/shared/lib/map/projection';
import { OSM_CONSTANTS } from '../model/osm-constants';
import type { MeterPos } from '@pixelet/shared/shared/model/coords';

const { unproject } = projection;
const { TILE_SIZE } = OSM_CONSTANTS;

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

/**
 * 특정 타일 인덱스의 미터 단위 BBox를 반환합니다.
 * * @param tx - 타일 X 인덱스
 * @param ty - 타일 Y 인덱스
 */
export const getTileBbox = (tx: number, ty: number): Bbox => {
  const { TILE_SIZE } = OSM_CONSTANTS;

  const minX = tx * TILE_SIZE;
  const minY = ty * TILE_SIZE;
  const maxX = (tx + 1) * TILE_SIZE;
  const maxY = (ty + 1) * TILE_SIZE;

  return { minX, minY, maxX, maxY };
};

export interface LocationBbox {
  minLat: number;
  minLng: number;
  maxLat: number;
  maxLng: number;
}

/**
 * [unproject-bbox]
 * 미터 단위의 BBox(X, Y)를 지리적 위경도 범위(LocationBbox)로 역투영합니다.
 * @param bbox - { minX, minY, maxX, maxY } 미터 단위 범위
 */
export const unprojectBbox = (bbox: Bbox): LocationBbox => {
  const { minX, minY, maxX, maxY } = bbox;

  // 1. 대각선 두 지점(SW, NE)을 위경도로 역투영
  // 지침 준수: XY 평면이 지표면이므로 z: 0을 입력
  const sw = unproject({ x: minX, y: minY, z: 0 });
  const ne = unproject({ x: maxX, y: maxY, z: 0 });

  // 2. 투영법에 따른 위경도 역전 현상 방지를 위해 Min/Max 재정렬
  return {
    minLat: Math.min(sw.lat, ne.lat),
    minLng: Math.min(sw.lng, ne.lng),
    maxLat: Math.max(sw.lat, ne.lat),
    maxLng: Math.max(sw.lng, ne.lng),
  };
};
