import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import type { Coord } from '@pixelet/shared/shared/model/coords';

const {
  CELL_SIZE: cellSize,
  Z_SCALE_RATIO: zScaleRatio,
  TILE_RESOLUTION: res,
} = OSM_CONSTANTS;

const PAD = 1;

/**
 * [bakeTileTerrain]
 * 패딩이 포함된 성긴 고도 샘플(35x35)을 타일 규격의 고해상도 복셀 그리드(2048x2048)로 변환합니다.
 * * * [Engineering Logic]
 * 1. Padding-Aware Bilinear Interpolation:
 * - 입력 데이터(points)는 보간 오차 방지를 위해 타일 경계 밖 128m 데이터(PAD=1)를 포함합니다.
 * - 타일의 실제 경계(0m, 4096m)를 샘플 인덱스의 [1]과 [innerSampleCount]에 정밀하게 매핑하여
 * 인접 타일 간의 지형 찢어짐(Seam)을 근본적으로 방지합니다.
 * * 2. Sampling Sync:
 * - 타일 해상도(res)를 샘플 구간 밀도와 동기화하여 데이터 포인트와 메쉬 정점을 1:1로 일치시킵니다.
 * * 3. Z-Quantization (Voxel Snap):
 * - 보간된 해발 고도(m)를 복셀 수직 단위(cellSize * zScaleRatio)로 나누어
 * 쉐이더에서 즉시 사용 가능한 수직 층(Floor) 인덱스로 변환합니다.
 *
 * @param points - South-to-North 순서로 정렬된 35x35 고도 샘플 (Padding 포함)
 * @returns 2048x2048 해상도의 Uint16 고도 인덱스 그리드 (Padding 제외)
 */
export function bakeTileTerrain(points: Coord[]): Uint16Array {
  const elevationGrid = new Uint16Array(res * res);

  // 복셀 한 칸의 실제 물리적 높이 (m)
  const voxelZUnit = cellSize * zScaleRatio;

  // 1. 패딩 규격 (128m 간격의 샘플 중 앞뒤 1개씩이 패딩)
  const length = points.length; // 35 x 35
  const sampleCountRaw = Math.sqrt(length); // 35

  // 🎯 [Engineering Fix] 정수가 아니라는 것은 격자 구조가 깨졌다는 뜻입니다.
  if (!Number.isInteger(sampleCountRaw)) {
    console.error(
      `[ElevationBaker] Invalid points length: ${length}. Must be a perfect square.`,
    );
    return elevationGrid; // 빈 그리드 반환하여 렌더링 충돌 방지
  }

  const sampleCount = Math.round(sampleCountRaw); // 35 = 33 + 2
  const innerSampleCount = sampleCount - PAD * 2; // 33 (실제 타일 너비: 4096 / 128 + 1)

  // 2D 배열 접근을 위한 헬퍼 (포인트 클라우드 인덱싱)
  // 💡 points는 row-major 순서로 정렬되어 있다고 가정합니다. (서버/클라이언트 모두 이 순서로 생성)
  const getSampleZ = (row: number, col: number) =>
    points[row * sampleCount + col].z;

  /**
   * [Main Grid Loop]
   * 타일 해상도(res)를 순회하며 각 픽셀의 복셀 층수를 결정합니다.
   */
  for (let gy = 0; gy < res; gy++) {
    for (let gx = 0; gx < res; gx++) {
      /**
       * [Step 1: Sampling Coordinate Mapping]
       * 타일 해상도(0~2047)를 샘플 인덱스(1~33) 사이로 매핑합니다.
       * 이렇게 해야 패딩(0번, 34번)을 '참조'하여 보간하되,
       * 타일의 끝점은 정확히 1번과 33번 샘플에 안착합니다.
       */
      const sy = PAD + (gy / (res - 1)) * (innerSampleCount - 1);
      const sx = PAD + (gx / (res - 1)) * (innerSampleCount - 1);

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
      elevationGrid[gy * res + gx] = Math.max(0, Math.min(zIndex, 65535));
    }
  }

  return elevationGrid;
}
