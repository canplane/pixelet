// @pixelet/web/src/entities/elevation/lib/elevation-baker.ts

import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import type { Coord } from '@pixelet/shared/shared/model/coords';

const {
  CELL_SIZE: cellSize,
  Z_SCALE_RATIO: zScaleRatio,
  TILE_RESOLUTION: resolution,
} = OSM_CONSTANTS;

/**
 * [bakeElevationGrid]
 * 성긴 고도 샘플(Vertex-based, 33x33)을 타일의 고해상도 복셀 그리드(2048x2048)로 변환합니다.
 * * [수학적 프로세스]
 * 1. Bilinear Interpolation: 128m 간격의 샘플 사이를 2m 간격의 부드러운 평면으로 보간합니다.
 * 2. Z-Quantization: 보간된 높이(m)를 복셀 수직 유닛(cellSize * zScaleRatio)으로 나누어 '층(Floor)' 인덱스로 변환합니다.
 * * @param points - 수집된 고도 데이터 (33x33 격자 권장)
 * @param options - tileSize(4096), cellSize(2), zScaleRatio(배율) 옵션
 * @returns Uint16Array - 각 셀의 복셀 층수(Index)가 담긴 1D 배열
 */
export function bakeElevationGrid(points: Coord[]): Uint16Array {
  const grid = new Uint16Array(resolution * resolution);

  // 복셀 한 칸의 실제 물리적 높이 (m)
  const voxelZUnit = cellSize * zScaleRatio;

  // 입력 데이터가 유효한 정사각 격자인지 검증 (예: 1089개 포인트 -> 33x33)
  const sampleCount = Math.sqrt(points.length);

  if (!Number.isInteger(sampleCount)) {
    console.error(
      `[ElevationBaker] 포인트 데이터가 정사각 격자 형태가 아닙니다. Length: ${points.length}`,
    );
    // 빈 그리드 반환하여 렌더링 충돌 방지
    return grid;
  }

  // 2D 배열 접근을 위한 헬퍼 (포인트 클라우드 인덱싱)
  // 💡 points는 row-major 순서로 정렬되어 있다고 가정합니다. (서버/클라이언트 모두 이 순서로 생성)
  const getSampleZ = (row: number, col: number) =>
    points[row * sampleCount + col].z;

  /**
   * [Main Grid Loop]
   * 타일 해상도(resolution)를 순회하며 각 픽셀의 복셀 층수를 결정합니다.
   */
  for (let gy = 0; gy < resolution; gy++) {
    for (let gx = 0; gx < resolution; gx++) {
      /**
       * [Step 1: Sampling Coordinate Mapping]
       * 출력 그리드 좌표(0~2047)를 샘플 그리드 좌표계(0.0~32.0)로 정밀하게 매핑합니다.
       * (resolution - 1)과 (sampleCount - 1)을 사용하여 타일의 양 끝점을 샘플 포인트와 일치시킵니다.
       */
      const sy = (gy / (resolution - 1)) * (sampleCount - 1);
      const sx = (gx / (resolution - 1)) * (sampleCount - 1);

      // 주변 4개 참조점(Vertex) 확정
      const y0 = Math.floor(sy);
      const y1 = Math.min(y0 + 1, sampleCount - 1);
      const x0 = Math.floor(sx);
      const x1 = Math.min(x0 + 1, sampleCount - 1);

      // 참조점 간 가중치 계산 (0.0 ~ 1.0)
      const dy = sy - y0;
      const dx = sx - x0;

      /**
       * [Step 2: Bilinear Interpolation]
       * 4개의 참조점 고도값을 선형 보간하여 현재 위치의 '연속적인 해발 고도'를 산출합니다.
       */
      const z =
        (1 - dx) * (1 - dy) * getSampleZ(y0, x0) +
        dx * (1 - dy) * getSampleZ(y0, x1) +
        (1 - dx) * dy * getSampleZ(y1, x0) +
        dx * dy * getSampleZ(y1, x1);

      /**
       * [Step 3: Voxel Snap (Quantization)]
       * 부드러운 고도값(m)을 복셀 수직 유닛으로 나누어 계단식 인덱스로 변환합니다.
       * * 계산 예 (voxelZUnit = 8m 기준):
       * - z = 15.0m -> floor(15.0 / 8.0) = 1 (1층)
       * - z = 24.5m -> floor(24.5 / 8.0) = 3 (3층)
       */
      const zIndex = Math.floor(z / voxelZUnit);

      /**
       * [Step 4: Storage]
       * Uint16(0~65535) 범위로 확장하여 저장합니다.
       * 값 '1'은 1층(voxelZUnit 미터)을 의미합니다.
       */
      grid[gy * resolution + gx] = Math.max(0, Math.min(zIndex, 65535));
    }
  }

  return grid;
}
