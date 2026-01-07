/**
 * @file chunks.ts
 * [Layer: BUILD] 청크(Chunk)의 데이터 구조(Model) 및 빌드 레이어 정의
 * * * [Architecture Note]
 * - 런타임에서 사용하는 3D 청크 객체의 모델을 정의합니다.
 * - 핵심 데이터 구조로 'SparseOctree'를 사용하여 부피(Volume) 데이터의 압축과 고속 접근을 보장합니다.
 * - Tile(BaseLayer)과 대칭되는 구조를 가지며, 편집 가능한 유저 창작 공간을 표현합니다.
 */

import { SparseOctree } from '@pixelet/core/shared/lib/structs/sparse-octree';
import type { ChunkCoord } from '../lib/utils/chunk-utils';
// import type { UserPoi } from './user-feature';

// =============================================================================
// 1. Channel Definitions (Volume Data)
// =============================================================================

/**
 * 🎯 [SSOT] 채널 키 정의
 * 현재는 기본 복셀 레이어만 정의하지만, 추후 조명(Light), 액체(Liquid) 등으로 확장 가능합니다.
 */
export const CHUNK_CHANNEL_KEYS = [
  'voxels', // 1. 기본 블록 ID (0: Air, 1~255: Block IDs)
  // 'meta',   // 2. (Optional) 회전, 상태 등 메타데이터
  // 'light',  // 3. (Optional) 구워진 조명 값
] as const;

export type ChunkChannelKey = (typeof CHUNK_CHANNEL_KEYS)[number];

/**
 * [ChunkContent]
 * 청크 내부를 구성하는 데이터 레이어들입니다. (Tile의 TileSurface와 대칭)
 * SparseOctree<Uint8Array>를 사용하여 64^3 공간을 효율적으로 관리합니다.
 * - 0 (Empty/Air) 값이 대부분인 청크의 특성상 메모리 사용량이 극적으로 감소합니다.
 */
export interface ChunkContent {
  /** * [Ch 1] 복셀 ID 데이터
   * - 0: Empty (Air)
   * - 1~255: Block Types
   */
  voxels: SparseOctree<Uint8Array>;
}

// =============================================================================
// 2. Chunk Definition (Unit)
// =============================================================================

/**
 * [Chunk]
 * 3D 사용자 데이터를 담는 공간 단위 (64x64x64)
 * 형상(Voxels) + 정보(POI) + 상태(State)를 모두 포함합니다.
 */
export interface Chunk {
  // --- Identity ---
  key: string; // "kx:ky:kz" (Chunk Key, using 'k' convention)
  coord: ChunkCoord; // { x, y, z } - Strict Integer Grid Coordinate

  // --- 1. Build Layer (User Content) ---
  /**
   * 유저가 편집한 3D 볼륨 데이터
   * - voxels: 복셀 ID (SparseOctree)
   */
  content: ChunkContent;

  // --- 2. Metadata (Info) ---
  /**
   * 청크 내에 포함된 사용자 정의 POI
   * (e.g., 표지판, 워프 포인트, 스폰 지점)
   */
  // pois: UserPoi[];

  // --- 3. State (Lifecycle) ---
  /** 청크의 현재 상태 및 권한 정보 */
  state: {
    isLoading: boolean; // 로딩 중 여부
    isError: boolean; // 로드 실패 여부
    isLocked: boolean; // 편집 잠금 (Admin/System)
    isVisible: boolean; // 렌더링 가시성

    ver: number; // 낙관적 락킹(Optimistic Locking)용 리비전 - Renamed from 'version'
    updatedAt: number; // 마지막 수정 타임스탬프 - Renamed from 'lastModified'
    permission: 'PUBLIC' | 'PRIVATE' | 'GUILD'; // 수정 권한 레벨
  };
}

// =============================================================================
// 3. BuildLayer Definition (Container)
// =============================================================================

/**
 * [BuildLayer]
 * 전역 사용자 창작물(World)을 관리하는 최상위 컨테이너 모델
 * - 런타임에 로드된 청크들의 집합을 관리합니다.
 */
export interface BuildLayer {
  /**
   * Loaded Chunks Map
   * - Key: "kx:ky:kz" -> O(1) Access
   * - Value: Chunk Instance
   */
  chunks: Map<string, Chunk>;

  /**
   * Layer Configuration (Global State)
   * - 에디터 및 뷰어의 전역 설정
   */
  config: {
    isVisible: boolean; // 레이어 전체 On/Off (Naming match: visible -> isVisible)
    opacity: number; // 투명도 (고스트 뷰 등)
    showGrid: boolean; // 청크 경계선 표시
    showOutlines: boolean; // 복셀 외곽선 강조
  };
}
