import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import type { Coord } from '@pixelet/shared/shared/model/coords';

const { CELL_SIZE, Z_UNIT_SCALE, TILE_RES, ELEVATION_SAMPLE_STEP } =
  OSM_CONSTANTS;

const PAD = 1;

/**
 * [bakeTileElevation]
 * 🎯 [Engineering Fix] Metric-Based Sampling
 * 기존의 인덱스 비율 방식(Stretching)을 버리고, 물리적 거리 기반으로 샘플링 좌표를 계산하여
 * 인접 타일 간의 지형 찢어짐(Seam)을 근본적으로 방지합니다.
 */
export function bakeTileElevation(points: Coord[]): Uint16Array {
  const elevationGrid = new Uint16Array(TILE_RES * TILE_RES);
  const voxelZUnit = CELL_SIZE / Z_UNIT_SCALE;

  const length = points.length;
  const sampleCountRaw = Math.sqrt(length);

  if (!Number.isInteger(sampleCountRaw)) {
    console.error(`[ElevationBaker] Invalid points length: ${length}`);
    return elevationGrid;
  }

  const sampleCount = Math.round(sampleCountRaw); // e.g. 17 (inner) + 2 (pad) = 19

  // 1D 배열을 2D처럼 접근 (Row-Major: South -> North)
  const getSampleZ = (row: number, col: number) => {
    // 🛡️ [Safety] Index Boundary Check
    if (row < 0) row = 0;
    if (row >= sampleCount) row = sampleCount - 1;
    if (col < 0) col = 0;
    if (col >= sampleCount) col = sampleCount - 1;

    return points[row * sampleCount + col].z;
  };

  for (let gy = 0; gy < TILE_RES; gy++) {
    for (let gx = 0; gx < TILE_RES; gx++) {
      // 1. Metric Coordinate (픽셀 중심점)
      const meterX = (gx + 0.5) * CELL_SIZE;
      const meterY = (gy + 0.5) * CELL_SIZE;

      // 2. Sample Index Mapping
      const sx = PAD + meterX / ELEVATION_SAMPLE_STEP;
      const sy = PAD + meterY / ELEVATION_SAMPLE_STEP;

      const x0 = Math.floor(sx);
      const x1 = x0 + 1;
      const y0 = Math.floor(sy);
      const y1 = y0 + 1;

      // 3. Bilinear Weights
      const dx = sx - x0;
      const dy = sy - y0;

      // 4. Interpolation
      const z =
        (1 - dx) * (1 - dy) * getSampleZ(y0, x0) +
        dx * (1 - dy) * getSampleZ(y0, x1) +
        (1 - dx) * dy * getSampleZ(y1, x0) +
        dx * dy * getSampleZ(y1, x1);

      const zIndex = Math.floor(z / voxelZUnit);
      elevationGrid[gy * TILE_RES + gx] = Math.max(0, Math.min(zIndex, 65535));
    }
  }

  return elevationGrid;
}
