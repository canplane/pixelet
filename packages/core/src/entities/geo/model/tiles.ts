/**
 * @file tiles.ts
 * [Layer: BASE] 타일(Tile)의 데이터 구조(Model) 및 베이스 레이어 정의
 * * * [Architecture Note]
 * - 런타임에서 사용하는 타일 객체의 모델을 정의합니다.
 * - 핵심 데이터 구조로 'SparseQuadtree'를 사용하여 메모리 효율과 접근 속도를 보장합니다.
 * - [STL Pattern] 별칭(Tree) 대신 표준 인터페이스 명칭을 사용하여 직관성을 높였습니다.
 */

import { SparseQuadtree } from '@pixelet/core/shared/lib/structs/sparse-quadtree';
import type { TileCoord } from '../lib/utils/tile-utils';
import type { MapPoi } from './osm-feature';

// =============================================================================
// 1. Channel Definitions (Skin)
// =============================================================================

export const TILE_SURFACE_KEYS = ['land', 'water', 'structure', 'transport'] as const;

export type TileSurfaceKey = (typeof TILE_SURFACE_KEYS)[number];

/**
 * [TileSurface]
 * 타일의 표면을 구성하는 텍스처 레이어들입니다.
 * 🚀 [Standardized] 밖에서 볼 때는 표준 TypedArray를 사용하는 것처럼 보이지만,
 * 내부적으로는 ArrayBufferLike 최적화가 적용된 상태입니다.
 */
export interface TileSurface {
  /** [Ch 1] 지형 (Land) */
  land: SparseQuadtree<Uint8Array>;
  /** [Ch 2] 내륙 수계 (Water) */
  water: SparseQuadtree<Uint8Array>;
  /** [Ch 3] 구조물 (Structure) */
  structure: SparseQuadtree<Uint8Array>;
  /** [Ch 4] 교통 (Transport) */
  transport: SparseQuadtree<Uint8Array>;
}

// =============================================================================
// 2. Tile Definition (Unit)
// =============================================================================

/**
 * [Tile]
 * 2.5D 지형 데이터를 담는 원자적 단위 (1024x1024)
 */
export interface Tile {
  key: string;
  coord: TileCoord;

  // --- 1. Base Layer (Bone & Soil) ---
  base: {
    surface: TileSurface;
    /** 지형의 고도 (Int16Array 기반) */
    ele: SparseQuadtree<Int16Array>;
  };

  // --- 2. Build Layer (User Content) ---
  build: Record<string, unknown>;

  // --- 3. Metadata (Info) ---
  pois: MapPoi[];

  // --- 4. Lifecycle/Version ---
  metadata: TileMetadata;
}

// =============================================================================
// 3. Transfer Model (DTO - Worker/Network)
// =============================================================================

/** * [TileDto] 데이터 전송 및 직렬화를 위한 경량 객체
 * 🚀 바이너리 데이터는 Codec을 통해 직렬화된 후 base64 또는 string 형태로 전달됩니다.
 */
export interface TileDto {
  key: string;
  coord: TileCoord;
  base: {
    /** [Ch 1~4] 직렬화된 SparseQuadtree (Binary Data) */
    surface: Record<TileSurfaceKey, string>;
    /** 직렬화된 고도 데이터 (Binary Data) */
    ele: string;
  };
  /** 직렬화된 POI 데이터 */
  pois: string;
  metadata: TileMetadata;
}

// =============================================================================
// 4. Shared Sub-types
// =============================================================================

export interface TileMetadata {
  ver: string;
  updatedAt: number;
}
