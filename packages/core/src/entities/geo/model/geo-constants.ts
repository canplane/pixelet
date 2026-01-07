/**
 * @file geo-constants.ts
 * [Core] 지형 엔진의 공간 규격, 격자 체계 및 데이터 처리 정책을 정의하는 핵심 상수입니다.
 * * [Data Hierarchy & Terminology]
 * 1. 🗺️ Tile: 최상위 공간 관리 단위 (Physical Area).
 * 2. 🧊 Chunk: 3D 사용자 공간 관리 단위 (Volume Area).
 * 3. 📡 Channel: 타일 내 속성 레이어 (Land, Water, Structure, Transport, Elevation).
 * 4. 📦 Sparse Block: Quadtree/Octree의 리프 노드 단위 (Memory Unit).
 * 5. 🧬 Cell: 엔진이 다루는 최소 데이터 입자 (Voxel/Pixel).
 *
 * * @description
 * 본 파일은 SSOT(Single Source of Truth)로서, 비트 연산 호환성을 최우선으로 고려하여
 * 해상도(Resolution)를 정의하고 이를 기반으로 물리적 크기(Size)를 유도합니다.
 */

// 🎯 1. Atomic Unit (기본 입자)
const _CELL_SIZE = 1; // 1 meter (Realistic Scale)

// 🎯 2. Resolution Constraints (Power of 2 Required)
// 비트 연산(Bitwise) 최적화를 위해 반드시 2의 거듭제곱이어야 합니다.
const _BLOCK_RES = 64; // 2^6 (Sparse Node Base)
const _TILE_RES = 256; // 2^10 (2D Tile Map)
const _CHUNK_RES = 64; // 2^6 (3D Chunk Volume)

const _LOAD_RADIUS = 8;

export const GEO_CONSTANTS = {
  /**
   * [Basic Unit]
   * 데이터의 최소 해상도 단위 (meters)
   */
  CELL_SIZE: _CELL_SIZE,

  /**
   * [Memory Unit]
   * Sparse Grid 구조를 최적화하기 위한 블록 규격 (Quadtree/Octree Base)
   */
  BLOCK_RES: _BLOCK_RES,

  // ===========================================================================
  // 1. Tile System (2D) - 🔒 Immutable Protocol
  // [주의] 이 섹션의 값들은 서버 데이터 규격과 직결되므로 디바이스별 변경이 불가합니다.
  // ===========================================================================

  /**
   * [2D Tile System]
   * * Goldilocks: 1024 Res (1km)
   * WebGPU 환경에서 Draw Call을 최소화하고 1km 관리 단위를 유지하기 위한 최적값입니다.
   * * @formula SIZE = RES * CELL_SIZE
   */
  TILE_RES: _TILE_RES, // 1024
  TILE_SHIFT: 8, // 2^10
  TILE_MASK: _TILE_RES - 1, // 1023 (0x3FF)
  TILE_SIZE: _TILE_RES * _CELL_SIZE, // 1024m (approx 1km)

  /**
   * [Tile Grid Count]
   * SparseQuadtree의 Macro Grid 해상도 (1024 / 64 = 16)
   * 16x16 그리드는 L1 캐시 적중률이 높으며, 빠른 컬링을 보장합니다.
   */
  TILE_GRID_COUNT: _TILE_RES / _BLOCK_RES,

  // ===========================================================================
  // 2. Chunk System (3D) - 🔒 Immutable Protocol
  // [주의] 이 섹션의 값들은 서버 데이터 규격과 직결되므로 디바이스별 변경이 불가합니다.
  // ===========================================================================

  /**
   * [3D Chunk System]
   * * Goldilocks: 64 Res (64m)
   * 3D 절두체 컬링(Frustum Culling) 효율과 메모리 파편화 방지를 위한 최적값입니다.
   * * @formula SIZE = RES * CELL_SIZE
   */
  CHUNK_RES: _CHUNK_RES, // 64
  CHUNK_SHIFT: 6, // 2^6
  CHUNK_MASK: _CHUNK_RES - 1, // 63 (0x3F)
  CHUNK_SIZE: _CHUNK_RES * _CELL_SIZE, // 64m

  /**
   * [Chunk Grid Count]
   * SparseOctree의 Macro Grid 해상도 (64 / 64 = 1)
   * 💡 현재 Chunk 크기와 Sparse Block 크기가 같으므로 Macro는 1x1x1입니다.
   * 이는 청크 전체가 "Uniform(Air/Solid)"인지 "Mixed(Detailed)"인지를
   * 단 하나의 Int16 값으로 판단할 수 있음을 의미합니다. (초고속 빈 청크 스킵)
   */
  CHUNK_GRID_COUNT: _CHUNK_RES / _BLOCK_RES,

  // ===========================================================================
  // 3. Policies & Scaling - 🎛️ Tunable Hyperparameters
  // [튜닝 가이드] 디바이스 성능(Mobile/Desktop)에 따라 이 값들을 런타임에 오버라이딩하세요.
  // ===========================================================================

  /**
   * [ELEVATION_SAMPLE_STEP] 지형 고도 샘플링 간격 (64m)
   * TILE_SIZE(1024)의 약수여야 타일 경계 오차를 방지할 수 있습니다.
   */
  ELEVATION_SAMPLE_STEP: 64,

  /** [Streaming & LOD Policy] */

  /**
   * [LOAD_RADIUS] 데이터 로드 반경
   * 플레이어 중심 타일로부터 데이터를 미리 가져오는(Fetch) 반경입니다.
   * * Default (Desktop): 2 (5x5 Grid, 약 2.5km 시야)
   * * Tuning (Mobile) : 1 (3x3 Grid, 약 1.5km 시야)
   */
  LOAD_RADIUS: _LOAD_RADIUS,

  /**
   * [CLEANUP_RADIUS] 메모리 해제 반경
   * 경계 지역에서 빈번한 로드/언로드를 방지(Hysteresis)합니다.
   * * Tuning: 보통 LOAD_RADIUS + 2 정도로 설정하여 안정성을 확보합니다.
   */
  CLEANUP_RADIUS: Math.round(_LOAD_RADIUS * 1.5),

  /**
   * [Z_UNIT_SCALE] 수직 해상도 및 왜곡 배율
   * 데이터 1유닛(Voxel Layer)이 물리적으로 담당하는 수직 거리(m)를 결정합니다.
   */
  Z_UNIT_SCALE: 1,

  /**
   * [REBASE_FLOATING_ORIGIN_THRES] 월드 앵커 재설정 임계값 (단위: Tile)
   * Floating Origin 시스템에서 좌표 정밀도를 유지하기 위해 월드 기준점(Anchor)을 다시 찍는 기준입니다.
   * * Tuning: GPU 정밀도가 낮은 모바일에서는 값을 낮춰(4~6) 더 자주 리셋할 수 있습니다.
   */
  REBASE_FLOATING_ORIGIN_THRES: 8,
} as const;
