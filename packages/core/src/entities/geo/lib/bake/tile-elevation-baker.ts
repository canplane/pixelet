/**
 * @file tile-elevation-baker.ts
 * 물리적 거리 기반 샘플링을 통해 고도 데이터를 SparseQuadtree 구조로 생성합니다.
 */

import { GEO_CONSTANTS } from '@pixelet/core/entities/geo/model/geo-constants';
import type { Coord } from '@pixelet/core/shared/model/coords';
// 🎯 표준 인터페이스 SparseQuadtree와 전용 생성 팩토리만 사용합니다.
import { SparseQuadtree } from '@pixelet/core/shared/lib/structs/sparse-quadtree';
import { createSparseQuadtree } from '@pixelet/core/entities/geo/lib/utils/tile-factory';

const { CELL_SIZE, Z_UNIT_SCALE, TILE_RES, ELEVATION_SAMPLE_STEP } = GEO_CONSTANTS;

const PAD = 1;

/**
 * [bakeTileElevation]
 * 🎯 [Engineering Fix] Metric-Based Sampling & [STL Adaptation]
 * 물리적 거리 기반 샘플링 로직을 유지하며,
 * 결과를 표준 SparseQuadtree 구조로 베이킹하여 메모리 효율을 극대화합니다.
 */
export function bakeTileElevation(points: Coord[]): SparseQuadtree<Int16Array> {
  // 1. [Initialization] 표준 팩토리를 통해 인스턴스 생성
  const tree = createSparseQuadtree(Int16Array);

  const voxelZUnit = CELL_SIZE / Z_UNIT_SCALE;
  const length = points.length;
  const sampleCountRaw = Math.sqrt(length);

  if (!Number.isInteger(sampleCountRaw)) {
    console.error(`[ElevationBaker] Invalid points length: ${length}`);
    return tree;
  }

  const sampleCount = Math.round(sampleCountRaw);

  /** 1D 배열을 2D처럼 접근 (Row-Major: South -> North) */
  const getSampleZ = (row: number, col: number) => {
    row = Math.max(0, Math.min(row, sampleCount - 1));
    col = Math.max(0, Math.min(col, sampleCount - 1));
    return points[row * sampleCount + col].z;
  };

  // 2. [Rasterization Loop] 1024x1024 격자 순회 및 선형 보간
  for (let gy = 0; gy < TILE_RES; gy++) {
    for (let gx = 0; gx < TILE_RES; gx++) {
      // 1) Metric Coordinate (픽셀 중심점 계산)
      const meterX = (gx + 0.5) * CELL_SIZE;
      const meterY = (gy + 0.5) * CELL_SIZE;

      // 2) Sample Index Mapping
      const sx = PAD + meterX / ELEVATION_SAMPLE_STEP;
      const sy = PAD + meterY / ELEVATION_SAMPLE_STEP;

      const x0 = Math.floor(sx),
        x1 = x0 + 1;
      const y0 = Math.floor(sy),
        y1 = y0 + 1;

      // 3) Bilinear Weights
      const dx = sx - x0;
      const dy = sy - y0;

      // 4) Interpolation (Bilinear)
      const z =
        (1 - dx) * (1 - dy) * getSampleZ(y0, x0) + dx * (1 - dy) * getSampleZ(y0, x1) + (1 - dx) * dy * getSampleZ(y1, x0) + dx * dy * getSampleZ(y1, x1);

      // 5) [Quantization] 정수형 고도값 변환 및 범위 보정
      const zIndex = Math.floor(z / voxelZUnit);
      const finalZ = Math.max(-32768, Math.min(zIndex, 32767));

      /** * 🚀 [Sparse Writing]
       * 내부의 Mixed Block 승격 로직은 SparseQuadtree 내부로 완전히 캡슐화되어 있습니다.
       */
      tree.set(gx, gy, finalZ);
    }
  }

  /** [Post-Optimization] 중복 블록 병합을 통한 메모리 압축 */
  tree.compact();

  return tree;
}
