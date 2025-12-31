/**
 * @file tile-math.ts
 * 타일 인덱스와 절대 좌표(Meter), 지리적 좌표(Location) 간의 변환을 담당합니다.
 */

import { projection } from '@pixelet/shared/shared/lib/map/projection';
import { OSM_CONSTANTS } from '../../model/osm-constants';
import type { Coord } from '@pixelet/shared/shared/model/coords';
import type {
  GridLayers,
  TileGrids,
} from '@pixelet/shared/entities/osm/model/tiles';

const { unproject } = projection;

const { TILE_SIZE, TILE_RESOLUTION } = OSM_CONSTANTS;

export interface TileCoord {
  tx: number;
  ty: number;
}

/**
 * 절대 좌표(Coord)를 받아 타일 인덱스(tx, ty)로 변환합니다.
 */
export const getTileCoord = (pos: Coord): TileCoord => {
  return {
    tx: Math.floor(pos.x / TILE_SIZE),
    ty: Math.floor(pos.y / TILE_SIZE),
  };
};

export const getTileKey = ({ tx, ty }: TileCoord) => `${tx}:${ty}`;

export interface Bbox {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
}

/**
 * 특정 타일 인덱스의 미터 단위 BBox를 반환합니다.
 * * @param coord - 타일 좌표
 */
export const getTileBbox = (coord: TileCoord): Bbox => {
  const { tx, ty } = coord;

  const minX = tx * TILE_SIZE;
  const minY = ty * TILE_SIZE;
  const maxX = (tx + 1) * TILE_SIZE;
  const maxY = (ty + 1) * TILE_SIZE;

  return { minX, minY, maxX, maxY };
};

export interface Bbox4326 {
  minLat: number;
  minLng: number;
  maxLat: number;
  maxLng: number;
}

/**
 * [unproject-bbox]
 * 미터 단위의 BBox(X, Y)를 지리적 위경도 범위(Bbox4326)로 역투영합니다.
 * @param bbox - { minX, minY, maxX, maxY } 미터 단위 범위
 */
export const unprojectBbox = (bbox: Bbox): Bbox4326 => {
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

/**
 * [GridLayers 초기화 팩토리]
 */
export function createEmptyLayers(
  resolution: number = TILE_RESOLUTION,
): GridLayers {
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
export function createEmptyGrids(
  resolution: number = TILE_RESOLUTION,
): TileGrids {
  const size = resolution * resolution;
  return {
    layers: createEmptyLayers(resolution),
    terrain: {
      // 🎯 고도 채널 추가: 기본값 0 (해수면 또는 기준 고도)
      height: new Uint16Array(size).fill(0),
    },
  };
}
