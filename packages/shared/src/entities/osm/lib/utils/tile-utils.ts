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

import { projection } from '@pixelet/shared/shared/lib/map/projection';
import { OSM_CONSTANTS } from '../../model/osm-constants';
import { CoordOps, type Coord2D } from '@pixelet/shared/shared/model/coords';
import {
  GRID_LAYER_KEYS,
  type GridLayers,
  type TileGrids,
} from '@pixelet/shared/entities/osm/model/tiles';

const { unproject } = projection;

const { TILE_SIZE, TILE_RESOLUTION } = OSM_CONSTANTS;

/**
 * [TileCoord]
 * 무한한 월드 공간을 유한한 타일 격자로 나누었을 때의 인덱스입니다.
 */
export interface TileCoord {
  tx: number;
  ty: number;
}

/**
 * [World -> Tile Index]
 * 절대 좌표(Meter)가 포함된 타일의 인덱스(tx, ty)를 계산합니다.
 * @example (1050m, 1050m) -> TileSize가 1000m라면 -> (1, 1)
 */
export const getTileCoord = (pos: Coord2D): TileCoord => {
  return {
    tx: Math.floor(pos.x / TILE_SIZE),
    ty: Math.floor(pos.y / TILE_SIZE),
  };
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
 * [Tile Absolute Anchor]
 * 타일 내 로컬 좌표 연산을 위한 절대 기준점(South-West / Min-Min)을 반환합니다.
 *
 * [Architectural Note]
 * 💡 'Anchor'는 Floating Origin 시스템이나 RTE(Relative To Eye) 렌더링에서
 * 거대한 절대 좌표를 작은 상대 좌표로 변환(Reduction)할 때 사용되는 핵심 기준점입니다.
 */
export const getTileAbsAnchor = (coord: TileCoord): Coord2D => {
  const bbox = getTileBbox(coord);
  return { x: bbox.minX, y: bbox.minY };
};

/**
 * [World -> Local Meter]
 * 절대 미터 좌표(AbsPos)를 특정 타일의 Anchor 기준 상대 좌표(Local Meter)로 변환합니다.
 *
 * * Formula: Local = Absolute - Anchor
 * * Usage: 타일 베이킹 시 좌표 정규화 등에 사용
 */
export const getTileLocalPos = (absPos: Coord2D, coord: TileCoord): Coord2D => {
  const anchor = getTileAbsAnchor(coord);
  return CoordOps.sub(absPos, anchor);
};

/**
 * [World -> Grid Index]
 * 절대 미터 좌표를 타일 내부의 그리드 인덱스(0 ~ Resolution)로 변환합니다.
 * 데이터 샘플링이나 충돌 처리에 사용됩니다.
 *
 * ⚠️ [Y-Axis Flip Warning]
 * - World Space: Y가 위로 갈수록 증가 (North)
 * - Grid/Image Space: Y가 아래로 갈수록 증가 (South/Top-Down)
 * - 따라서 변환 과정에서 `TILE_SIZE - localY` 연산이 수행됩니다.
 */
export const getTileLocalGridIdx = (
  absPos: Coord2D,
  coord: TileCoord,
  resolution: number = TILE_RESOLUTION,
): { gx: number; gy: number } => {
  const localPos = getTileLocalPos(absPos, coord);
  const cellSize = TILE_SIZE / resolution;

  return {
    // 서쪽에서 동쪽으로 (+X)
    gx: Math.floor(localPos.x / cellSize),
    // 북쪽(TILE_SIZE)에서 남쪽으로 하강 (Y-Flip)
    gy: Math.floor((TILE_SIZE - localPos.y) / cellSize),
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
 * [Layers Factory]
 * 🚀 스키마(GRID_LAYER_KEYS)에 정의된 모든 레이어(8-bit)를 자동으로 생성합니다.
 * 메모리 효율을 위해 Uint8Array를 사용합니다.
 */
export function createEmptyLayers(
  resolution: number = TILE_RESOLUTION,
): GridLayers {
  const size = resolution * resolution;

  const entries = GRID_LAYER_KEYS.map((key) => [key, new Uint8Array(size)]);

  return Object.fromEntries(entries) as unknown as GridLayers;
}

/**
 * [Tile Grids Factory] 빈 그리드 생성기
 * 모든 채널을 동일한 해상도로 생성하며, 지형 데이터(elevation)는 정밀도를 위해 16-bit을 사용합니다.
 */
export function createEmptyGrids(
  resolution: number = TILE_RESOLUTION,
): TileGrids {
  return {
    layers: createEmptyLayers(resolution),
    terrain: {
      // 고도(Elevation) 데이터는 m 단위 정밀도 표현을 위해 16-bit 사용 (기본값 0: 해수면)
      elevation: new Uint16Array(resolution * resolution).fill(0),
    },
  };
}

/**
 * [Pixel -> World] Debug Utility
 * 타일 내 로컬 픽셀 좌표(Grid Index)를 다시 절대 미터 좌표로 복원합니다.
 * 쉐이더 디버깅이나 마우스 피킹(Picking) 역산 시 유용합니다.
 */
export const pixelToWorld = (
  coord: TileCoord,
  localPos: Coord2D,
  resolution: number = TILE_RESOLUTION,
): Coord2D => {
  const bbox = getTileBbox(coord);
  const cellSize = TILE_SIZE / resolution;
  return {
    // 서쪽(minX)에서 동쪽으로 이동
    x: bbox.minX + localPos.x * cellSize,
    // [Top-Down 보정 복원] 북쪽(maxY)에서 남쪽으로 하강했던 것을 다시 미터 좌표계로 복구
    y: bbox.maxY - localPos.y * cellSize,
  };
};

/**
 * [World -> Pixel] Debug Utility
 * 절대 미터 좌표가 타일 내의 정확히 어느 픽셀(소수점 포함)에 위치하는지 계산합니다.
 * `getTileLocalGridIdx`와 유사하지만, 정수형 인덱스(Floor)가 아닌 실수형 좌표를 반환합니다.
 */
export const worldToPixel = (
  coord: TileCoord,
  worldPos: Coord2D,
  resolution: number = TILE_RESOLUTION,
): { localX: number; localY: number } => {
  const bbox = getTileBbox(coord);
  const cellSize = TILE_SIZE / resolution;

  return {
    // 서쪽 경계(minX)로부터의 오프셋을 cellSize로 나눔
    localX: (worldPos.x - bbox.minX) / cellSize,
    // 북쪽 경계(maxY)로부터 하강하는 오프셋을 cellSize로 나눔 (Top-Down)
    localY: (bbox.maxY - worldPos.y) / cellSize,
  };
};
