/**
 * @file tile-math.ts
 * 📐 [Tile Coordinate System Core]
 * 타일 인덱싱, 절대 미터 좌표(EPSG:3857), 지리적 좌표(EPSG:4326), 그리고 로컬 그리드 좌표 간의
 * 수학적 변환 공식들을 담당하는 순수 함수 집합입니다.
 *
 * [System Coordinate Conventions]
 * 1. World Coord (Meter): X(East), Y(North) - Bottom-Up
 * 2. Tile Index: TX(East), TY(North) - Bottom-Up
 * 3. Grid/Pixel Coord: GX(East), GY(South) - Top-Down (텍스처/이미지 표준)
 *
 * 💡 주의: World(2D) -> Grid(2D) 변환 시 Y축 반전(Y-Flip)이 발생하므로 주의가 필요합니다.
 */

import { projection } from '@pixelet/core/shared/lib/map/projection';
import { GEO_CONSTANTS } from '../../model/geo-constants';
import {
  CoordOps,
  type Coord,
  type Coord2D,
} from '@pixelet/core/shared/model/coords';
import {
  TILE_CHANNEL_KEYS,
  type Tile,
  type TileTerrain,
  type TileChannels,
} from '@pixelet/core/entities/geo/model/tiles';

const { unproject } = projection;

const { TILE_SIZE, CELL_SIZE, TILE_RES } = GEO_CONSTANTS;

/**
 * [TileCoord]
 * 무한한 월드 공간을 유한한 타일 격자로 나누었을 때의 인덱스입니다.
 */
export interface TileCoord {
  tx: number;
  ty: number;
}

export interface CellCoord {
  cx: number;
  cy: number;
}

/**
 * [World -> Tile Index]
 * 절대 좌표(Meter)가 포함된 타일의 인덱스(tx, ty)를 계산합니다.
 * @example (1050m, 1050m) -> TileSize가 1000m라면 -> (1, 1)
 */
export const getTileCoord = (globalPos: Coord2D): TileCoord => {
  return {
    tx: Math.floor(globalPos.x / TILE_SIZE),
    ty: Math.floor(globalPos.y / TILE_SIZE),
  };
};

export const getTileCenter = (tileCoord: TileCoord): Coord2D => {
  const { x: ax, y: ay } = getTileOrigin(tileCoord);
  return {
    x: ax + TILE_SIZE / 2,
    y: ay + TILE_SIZE / 2,
  };
};

export const getTileDistance = (
  dstTileCoord: TileCoord,
  srcTileCoord: TileCoord,
): number => {
  const dTx = dstTileCoord.tx - srcTileCoord.tx;
  const dTy = dstTileCoord.ty - srcTileCoord.ty;
  return dTx ** 2 + dTy ** 2;
};

/**
 * [Tile Key Generator]
 * 타일 식별을 위한 고유 문자열 키를 생성합니다. (Map의 Key로 사용)
 * Format: "tx:ty"
 */
export const getTileKey = ({ tx, ty }: TileCoord) => `${tx}:${ty}`;

export interface Bbox {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
}

/**
 * [Tile Bounding Box]
 * 특정 타일 인덱스가 커버하는 절대 미터 좌표 범위(Boundary)를 반환합니다.
 * 이 BBox는 타일 내부 오프셋 계산의 기준이 됩니다.
 */
export const getTileBbox = (coord: TileCoord): Bbox => {
  const { tx, ty } = coord;

  const minX = tx * TILE_SIZE;
  const minY = ty * TILE_SIZE;
  const maxX = (tx + 1) * TILE_SIZE;
  const maxY = (ty + 1) * TILE_SIZE;

  return { minX, minY, maxX, maxY };
};

/**
 * [getTileOrigin]
 * 타일 내 로컬 좌표 연산을 위한 절대 기준점(South-West / Min-Min)을 반환합니다.
 *
 * [Architectural Note]
 * 💡 'Anchor'는 Floating Origin 시스템이나 RTE(Relative To Eye) 렌더링에서
 * 거대한 절대 좌표를 작은 상대 좌표로 변환(Reduction)할 때 사용되는 핵심 기준점입니다.
 */
export const getTileOrigin = (tileCoord: TileCoord): Coord2D => {
  const bbox = getTileBbox(tileCoord);
  return { x: bbox.minX, y: bbox.minY };
};

/**
 * [tileOffsetToCell]
 * cy=0은 타일의 남쪽(MinY), cy=99는 타일의 북쪽(MaxY)이 됩니다.
 */
export const tileOffsetToCell = (tileOffset: Coord2D): CellCoord => {
  return {
    cx: Math.floor(tileOffset.x / CELL_SIZE),
    cy: Math.floor(tileOffset.y / CELL_SIZE),
  };
};

/**
 * [cellToTileOffset]
 * 💡 셀의 '정중앙' 좌표를 반환하도록 설계되었습니다 (+0.5).
 */
export const cellToTileOffset = (cellCoord: CellCoord): Coord2D => {
  return {
    x: cellCoord.cx * CELL_SIZE,
    y: cellCoord.cy * CELL_SIZE,
  };
};

export interface Bbox4326 {
  minLat: number;
  minLng: number;
  maxLat: number;
  maxLng: number;
}

/**
 * [Unproject BBox] (Meter -> Lat/Lon)
 * 미터 단위의 BBox(X, Y)를 지리적 위경도 범위(Bbox4326)로 역투영합니다.
 * 주로 외부 API(Overpass 등)에 데이터를 요청할 때 쿼리 범위로 사용됩니다.
 * (예시: `${south},${west},${north},${east}`)
 */
export const unprojectBbox = (bbox: Bbox): Bbox4326 => {
  const { minX, minY, maxX, maxY } = bbox;

  // 1. 대각선 두 지점(SW, NE)을 위경도로 역투영
  // 지침 준수: XY 평면이 지표면이므로 z: 0을 입력
  const sw = unproject({ x: minX, y: minY, z: 0 });
  const ne = unproject({ x: maxX, y: maxY, z: 0 });

  // 2. 투영법에 따른 위경도 역전 현상 방지를 위해 Min/Max 재정렬
  return {
    minLat: Math.min(sw.lat, ne.lat), // south
    minLng: Math.min(sw.lng, ne.lng), // west
    maxLat: Math.max(sw.lat, ne.lat), // north
    maxLng: Math.max(sw.lng, ne.lng), // east
  };
};

/**
 * [Tile Channels Factory]
 * 🚀 스키마(TILE_CHANNEL_KEYS)에 정의된 모든 채널(8-bit)을 자동으로 생성합니다.
 * 메모리 효율을 위해 Uint8Array를 사용합니다.
 */
export function createEmptyChannels(): TileChannels {
  const size = TILE_RES * TILE_RES;

  const entries = TILE_CHANNEL_KEYS.map((key) => [key, new Uint8Array(size)]);

  return Object.fromEntries(entries) as unknown as TileChannels;
}

/**
 * [Tile Terrain Factory]
 * 모든 채널을 동일한 해상도로 생성하며, 지형 데이터(elevation)는 정밀도를 위해 16-bit을 사용합니다.
 */
export function createEmptyTerrain(): TileTerrain {
  const size = TILE_RES * TILE_RES;

  return {
    channels: createEmptyChannels(),
    // 고도(Elevation) 데이터는 m 단위 정밀도 표현을 위해 16-bit 사용 (기본값 0: 해수면)
    elevation: new Uint16Array(size).fill(0),
  };
}

/**
 * [Tile Factory]
 */
export function createEmptyTile(coord: TileCoord): Tile {
  return {
    key: getTileKey(coord),
    coord,
    terrain: createEmptyTerrain(),
    pois: [],
  };
}

export const worldToTile = (globalPos: Coord2D) => {
  const tileCoord = getTileCoord(globalPos);
  const tileOrigin = getTileOrigin(tileCoord);
  const tileOffset = CoordOps.sub(globalPos, tileOrigin);
  const cellCoord = {
    // 서쪽 경계(minX)로부터의 거리
    cx: tileOffset.x / CELL_SIZE,
    // 남쪽 경계(minY)로부터의 거리 (No Flip)
    cy: tileOffset.y / CELL_SIZE,
  };
  return { tileCoord, cellCoord, tileOrigin, tileOffset };
};

/**
 * [World -> Tile Offset]
 * 절대 미터 좌표(AbsPos)를 특정 타일의 Anchor 기준 상대 좌표(tileOffset)로 변환합니다.
 *
 * * Formula: Local = Absolute - Anchor
 * * Usage: 타일 베이킹 시 좌표 정규화 등에 사용
 */
export const worldToTileOffset = (globalPos: Coord2D): Coord2D => {
  const tileCoord = getTileCoord(globalPos);
  const tileOrigin = getTileOrigin(tileCoord);
  return CoordOps.sub(globalPos, tileOrigin);
};

/**
 * [World -> Cell]
 * 절대 미터 좌표가 타일 내의 정확히 어느 셀(실수형)에 위치하는지 계산합니다.
 * 💡 데이터 배열과 동일한 Bottom-Up 방향을 유지합니다.
 */
export const worldToCell = (globalPos: Coord2D): CellCoord => {
  const tileOffset = worldToTileOffset(globalPos);
  return {
    // 서쪽 경계(minX)로부터의 거리
    cx: tileOffset.x / CELL_SIZE,
    // 남쪽 경계(minY)로부터의 거리 (No Flip)
    cy: tileOffset.y / CELL_SIZE,
  };
};

/**
 * [Cell -> World]
 * 타일 내 셀 좌표를 다시 절대 미터 좌표로 복원합니다.
 */
export const cellToWorld = (
  tileCoord: TileCoord,
  cellCoord: CellCoord,
): Coord2D => {
  const bbox = getTileBbox(tileCoord);
  return {
    x: bbox.minX + cellCoord.cx * CELL_SIZE,
    y: bbox.minY + cellCoord.cy * CELL_SIZE,
  };
};

/**
 * [Get Elevation at World Pos]
 * 특정 월드 좌표의 고도값을 elevation 배열에서 즉시 추출합니다.
 */
export const getTerrainAltitudeCoord = (
  tile: Tile,
  cellCoord: CellCoord,
): number => {
  const { cx, cy } = cellCoord;

  // // 경계값 보호
  // if (cx < 0 || cx >= TILE_RES || cy < 0 || cy >= TILE_RES) {
  //   return 0;
  // }

  // 인덱스 범위 가드 (0 ~ TILE_RES - 1)
  const ix = Math.max(0, Math.min(TILE_RES - 1, Math.floor(cx)));
  const iy = Math.max(0, Math.min(TILE_RES - 1, Math.floor(cy)));

  // 🎯 데이터 배열 인덱싱: y * width + x
  return tile.terrain.elevation[iy * TILE_RES + ix];
};

export const clampPrecision = (val: number) => Math.round(val * 1e6) / 1e6;

export const toFixedCoord = (coord: Coord): Coord => ({
  x: clampPrecision(coord.x),
  y: clampPrecision(coord.y),
  z: clampPrecision(coord.z ?? 0),
});

/**
 * [snapToCell]
 * 물리 좌표를 셀 단위로 스냅합니다.
 * Floating Origin 리셋 시 이 단위를 기준으로 이동해야 배경 격자가 튀지 않습니다.
 * 💡 이제 입력된 pos.y는 북향(North) 좌표입니다.
 */
export const snapToCell = (pos: Coord, size: number = CELL_SIZE): Coord => {
  return {
    x: Math.floor(pos.x / size) * size,
    y: Math.floor(pos.y / size) * size, // 북향 스냅
    z: Math.floor(pos.z / size) * size, // 고도 스냅
  };
};

/** 셀 중앙 좌표로 보정합니다. */
export const toCenter = <T extends number | Coord2D | Coord>(
  input: T,
  size: number = CELL_SIZE,
): T => {
  const half = size / 2;

  // 1. 단순 숫자인 경우
  if (typeof input === 'number') {
    return (input + half) as T;
  }

  // 2. 좌표 객체인 경우 (Coord2D | Coord)
  // CoordOps.add가 제네릭 T를 반환하므로 타입 캐스팅 없이 안전하게 환원됩니다.
  return CoordOps.add(input as Coord2D | Coord, half) as T;
};
