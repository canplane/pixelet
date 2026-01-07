/**
 * @file tile-factory.ts
 * [Layer: GEO] Tile 및 하위 레이어(Base)의 초기 인스턴스를 생성하는 팩토리입니다.
 * [Architecture]
 * - SparseQuadtree(L1: Macro, L2: Details) 구조를 기반으로 메모리 효율적인 초기 상태를 생성합니다.
 * - 모든 수치와 규격은 geo-constants.ts의 SSOT를 준수하며, 비트 연산에 최적화된 블록 크기를 사용합니다.
 */

import { GEO_CONSTANTS } from '../../model/geo-constants';
import { Tile, TILE_SURFACE_KEYS, TileDto, TileSurface, TileSurfaceKey } from '../../model/tiles';
import { getTileKey, TileCoord } from '../utils/tile-utils';
import { SparseQuadtree } from '@pixelet/core/shared/lib/structs/sparse-quadtree';
import { TypedArray, TypedArrayConstructor } from '@pixelet/core/shared/lib/structs/types';

/**
 * 🏭 [Universal Factory]
 * STL 스타일로 통합된 SparseQuadtree 인스턴스를 생성합니다.
 * @param Ctor 표준 TypedArray 생성자 (Int16Array, Uint8Array 등)
 * @param defaultValue 초기 Uniform 값
 * @param size 트리 크기 (기본값: SSOT 준수)
 */
export function createSparseQuadtree<T extends TypedArray>(
  Ctor: TypedArrayConstructor<T>,
  defaultValue: number = 0,
  size: number = GEO_CONSTANTS.TILE_RES,
): SparseQuadtree<T> {
  // 1. SparseQuadtree 생성
  const tree = new SparseQuadtree(size, Ctor);

  // 2. 초기값 설정
  if (defaultValue !== 0) {
    tree.macro.fill(defaultValue);
  }

  return tree;
}

/** * 2. BASE 레이어 생성
 * 이제 파편화된 별칭들을 모두 제거하고 표준 명칭으로 통일합니다.
 */
export function createEmptyBase() {
  const surface = TILE_SURFACE_KEYS.reduce((acc, key) => {
    // 🎯 SparseQuadtree<Uint8Array> 타입으로 자동 추론됩니다.
    acc[key] = createSparseQuadtree(Uint8Array, 0);
    return acc;
  }, {} as Partial<TileSurface>) as TileSurface;

  return {
    surface,
    // 🎯 SparseQuadtree<Int16Array> 타입으로 자동 추론됩니다.
    ele: createSparseQuadtree(Int16Array, 0),
  };
}

/** * 3. 타일 객체 조립 (Runtime Model) */
export function createEmptyTile(coord: TileCoord): Tile {
  return {
    key: getTileKey(coord),
    coord,
    base: createEmptyBase(),
    build: {},
    pois: [],
    metadata: {
      ver: '1.0.0',
      updatedAt: Date.now(),
    },
  };
}

/** * 4. TileDto 생성 (Transfer Model) */
export function createEmptyTileDto(coord: TileCoord): TileDto {
  const emptySurface = TILE_SURFACE_KEYS.reduce(
    (acc, key) => {
      acc[key] = '';
      return acc;
    },
    {} as Partial<Record<TileSurfaceKey, string>>,
  ) as Record<TileSurfaceKey, string>;

  return {
    key: getTileKey(coord),
    coord,
    base: {
      surface: emptySurface,
      ele: '',
    },
    pois: '',
    metadata: {
      ver: '1.0.0',
      updatedAt: Date.now(),
    },
  };
}
