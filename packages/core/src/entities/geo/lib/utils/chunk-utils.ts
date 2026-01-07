/**
 * @file chunk-utils.ts
 * 🧊 [Chunk Coordinate System Core]
 * 3D 청크 그리드 시스템을 위한 수학적 변환 공식입니다.
 *
 * * [Convention]
 * - Global: x, y, z (meters)
 * - Chunk: cx, cy, cz (indices)
 * - Cell: ox, oy, oz (offsets/indices within chunk, 0~63)
 *
 * * [Optimization]
 * - GC 방지를 위해 선택적 'out' 파라미터 패턴(gl-matrix style)을 지원합니다.
 * - TileUtils와 완벽한 대칭 구조(Symmetry)를 가집니다.
 */

import { GEO_CONSTANTS } from '../../model/geo-constants';
import { CoordOps, type Coord } from '@pixelet/core/shared/model/coords';

// 🎯 SSOT Constants
const { CHUNK_SIZE, CELL_SIZE, CHUNK_RES, CHUNK_SHIFT, CHUNK_MASK } = GEO_CONSTANTS;

// =============================================================================
// 1. Types (Strict Property Naming)
// =============================================================================

export interface ChunkCoord {
  cx: number;
  cy: number;
  cz: number;
}

export interface VoxelCoord {
  ox: number; // Offset X (0 ~ 63)
  oy: number; // Offset Y (0 ~ 63)
  oz: number; // Offset Z (0 ~ 63)
}

export interface Bbox3D {
  minX: number;
  minY: number;
  minZ: number;
  maxX: number;
  maxY: number;
  maxZ: number;
}

// =============================================================================
// 2. Converters (Global <-> Chunk)
// =============================================================================

/**
 * [Global -> Chunk]
 * 절대 좌표가 속한 청크 인덱스를 계산합니다.
 */
export const getChunkCoord = (pos: Coord, out?: ChunkCoord): ChunkCoord => {
  const cx = Math.floor(pos.x / CHUNK_SIZE);
  const cy = Math.floor(pos.y / CHUNK_SIZE);
  const cz = Math.floor(pos.z / CHUNK_SIZE);

  if (out) {
    out.cx = cx;
    out.cy = cy;
    out.cz = cz;
    return out;
  }
  return { cx, cy, cz };
};

/**
 * [Chunk -> Global Origin]
 * 청크의 원점(Min-Corner) 절대 좌표를 반환합니다.
 */
export const getChunkOrigin = (coord: ChunkCoord, out?: Coord): Coord => {
  const x = coord.cx * CHUNK_SIZE;
  const y = coord.cy * CHUNK_SIZE;
  const z = coord.cz * CHUNK_SIZE;

  if (out) {
    out.x = x;
    out.y = y;
    out.z = z;
    return out;
  }
  return { x, y, z };
};

/**
 * [Chunk -> Global Center]
 */
export const getChunkCenter = (coord: ChunkCoord, out?: Coord): Coord => {
  const res = getChunkOrigin(coord, out);
  const half = CHUNK_SIZE / 2;
  res.x += half;
  res.y += half;
  res.z += half;
  return res;
};

/**
 * [Chunk -> BBox3D]
 */
export const getChunkBbox = (coord: ChunkCoord, out?: Bbox3D): Bbox3D => {
  const minX = coord.cx * CHUNK_SIZE;
  const minY = coord.cy * CHUNK_SIZE;
  const minZ = coord.cz * CHUNK_SIZE;

  if (out) {
    out.minX = minX;
    out.minY = minY;
    out.minZ = minZ;
    out.maxX = minX + CHUNK_SIZE;
    out.maxY = minY + CHUNK_SIZE;
    out.maxZ = minZ + CHUNK_SIZE;
    return out;
  }
  return {
    minX,
    minY,
    minZ,
    maxX: minX + CHUNK_SIZE,
    maxY: minY + CHUNK_SIZE,
    maxZ: minZ + CHUNK_SIZE,
  };
};

// =============================================================================
// 3. Converters (Global <-> Voxel/Offset)
// =============================================================================

/**
 * [Global -> Voxel Offset]
 * 절대 좌표를 청크 내 오프셋(0 ~ 63)으로 변환합니다.
 */
export const worldToChunkOffset = (pos: Coord, out?: VoxelCoord): VoxelCoord => {
  const cx = Math.floor(pos.x / CHUNK_SIZE);
  const cy = Math.floor(pos.y / CHUNK_SIZE);
  const cz = Math.floor(pos.z / CHUNK_SIZE);

  const ox = pos.x - cx * CHUNK_SIZE;
  const oy = pos.y - cy * CHUNK_SIZE;
  const oz = pos.z - cz * CHUNK_SIZE;

  if (out) {
    out.ox = ox;
    out.oy = oy;
    out.oz = oz;
    return out;
  }
  return { ox, oy, oz };
};

/**
 * [Voxel Offset -> Global]
 */
export const voxelToWorld = (cCoord: ChunkCoord, vCoord: VoxelCoord, out?: Coord): Coord => {
  const x = cCoord.cx * CHUNK_SIZE + vCoord.ox * CELL_SIZE;
  const y = cCoord.cy * CHUNK_SIZE + vCoord.oy * CELL_SIZE;
  const z = cCoord.cz * CHUNK_SIZE + vCoord.oz * CELL_SIZE;

  if (out) {
    out.x = x;
    out.y = y;
    out.z = z;
    return out;
  }
  return { x, y, z };
};

// =============================================================================
// 4. Utilities
// =============================================================================

export const getChunkKey = (coord: ChunkCoord): string => `${coord.cx}:${coord.cy}:${coord.cz}`;

export const parseChunkKey = (key: string, out?: ChunkCoord): ChunkCoord => {
  const [cx, cy, cz] = key.split(':').map(Number);
  if (out) {
    out.cx = cx;
    out.cy = cy;
    out.cz = cz;
    return out;
  }
  return { cx, cy, cz };
};

export const getChunkDistance = (a: ChunkCoord, b: ChunkCoord): number => {
  const dx = a.cx - b.cx;
  const dy = a.cy - b.cy;
  const dz = a.cz - b.cz;
  return dx * dx + dy * dy + dz * dz;
};

/**
 * 🛡️ [Boundary Check]
 */
export const isOutOfBounds = (ox: number, oy: number, oz: number): boolean => {
  return (ox & ~CHUNK_MASK) !== 0 || (oy & ~CHUNK_MASK) !== 0 || (oz & ~CHUNK_MASK) !== 0;
};

// --- Clamping ---

export const clampVoxel = (val: number): number => Math.max(0, Math.min(CHUNK_MASK, Math.floor(val)));

export const clampVoxelCoord = (coord: VoxelCoord, out?: VoxelCoord): VoxelCoord => {
  const ox = clampVoxel(coord.ox);
  const oy = clampVoxel(coord.oy);
  const oz = clampVoxel(coord.oz);

  if (out) {
    out.ox = ox;
    out.oy = oy;
    out.oz = oz;
    return out;
  }
  return { ox, oy, oz };
};
