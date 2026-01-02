import type { Coord4326, Coord } from '@pixelet/shared/shared/model/coords';
import type { Bbox } from '@pixelet/shared/entities/osm/lib/utils/tile-utils';
import { projection } from '@pixelet/shared/shared/lib/map/projection';
import { withSharedPromise } from '@pixelet/shared/shared/lib/cache-utils';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';

const { ELEVATION_SAMPLE_STEP } = OSM_CONSTANTS;
const PAD = 1;

interface ElevationElement {
  latitude: number;
  longitude: number;
  elevation: number;
}

interface ElevationApiResponse {
  results: ElevationElement[];
  status: string;
}

const { unproject } = projection;

/**
 * [Internal] 실제 API 서버에 대량의 좌표를 전달하여 고도를 응답받습니다.
 */
async function requestElevationBatch(
  points: Coord4326[],
): Promise<ElevationElement[]> {
  const locations = points.map((p) => ({
    latitude: p.lat,
    longitude: p.lng,
  }));

  const response = await fetch('/elevation-proxy/api/v1/lookup', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ locations }),
  });

  if (!response.ok) {
    const errorText = await response.text();
    console.error('Elevation API Raw Error:', errorText);
    throw new Error(`HTTP ${response.status}: ${errorText}`);
  }

  const data = (await response.json()) as ElevationApiResponse;
  return data.results as ElevationElement[];
}

/**
 * [requestElevation]
 * GeoRepository(PostGIS)의 로직과 수학적으로 1:1 매칭되는 엄밀한(Rigorous) 구현입니다.
 *
 * @verification
 * 1. Grid Alignment: PostGIS의 Half-Pixel Shift된 래스터 픽셀 중앙값 좌표와 정확히 일치하는 World Coord를 계산합니다.
 * 2. Ordering: PostGIS 덤프 순서와 동일하게 South-to-North(Row 역순)로 순회하여, 배열 인덱스 0이 South-West가 되도록 보장합니다.
 * 3. Padding: 타일 경계 보간 오차 제거를 위한 상하좌우 1 Step 확장을 동일하게 수행합니다.
 */
export async function requestElevation(
  bbox: Bbox,
  step: number = ELEVATION_SAMPLE_STEP,
): Promise<Coord[]> {
  // 캐시 키 생성: PostGIS 쿼리 파라미터와 동일한 스펙
  const rawKey = `elevation:${bbox.minX},${bbox.minY}:${step}`;

  return withSharedPromise('elevation', rawKey, async () => {
    const widthMeters = bbox.maxX - bbox.minX;
    const heightMeters = bbox.maxY - bbox.minY;

    // 1. Grid Size Calculation
    // GeoRepository와 동일하게 Math.round를 사용하여 격자 크기를 계산 (Floor 아님)
    const innerWidth = Math.round(widthMeters / step) + 1;
    const innerHeight = Math.round(heightMeters / step) + 1;

    // Padding 적용 (상하좌우 1칸)
    const paddedWidth = innerWidth + PAD * 2;
    const paddedHeight = innerHeight + PAD * 2;

    // 2. Logical Coordinates Origin
    const paddedMinX = bbox.minX - step * PAD;
    const paddedMaxY = bbox.maxY + step * PAD;

    // 3. Coordinate Generation (South-to-North Order)
    // GeoRepository의 `for (let row = paddedHeight - 1; row >= 0; row--)` 루프와 동일한 순서.
    // Row가 감소함에 따라 Y좌표는 남쪽(MinY)에서 북쪽(MaxY)으로 이동하지 않지만,
    // 배열에 push되는 순서는 "가장 남쪽 Row"부터 시작됩니다.
    // 즉, Index 0 = Bottom-Left (South-West)가 됩니다.
    const pointsToFetch: { pos: Coord; location: Coord4326 }[] = [];

    for (let row = paddedHeight - 1; row >= 0; row--) {
      // Y축: paddedMaxY(Top)에서 시작하여 step만큼 내려옴
      // row가 paddedHeight-1일 때, worldY는 가장 낮은 값(South)을 가짐
      const worldY = paddedMaxY - row * step;

      for (let col = 0; col < paddedWidth; col++) {
        // X축: paddedMinX(Left)에서 시작하여 step만큼 증가
        // PostGIS 래스터의 픽셀 센터(Vertex)와 정확히 일치하는 좌표
        const worldX = paddedMinX + col * step;

        const { lat, lng } = unproject({ x: worldX, y: worldY, z: 0 });
        pointsToFetch.push({
          pos: { x: worldX, y: worldY, z: 0 },
          location: { lat, lng },
        });
      }
    }

    // 4. Batch Request (API 호출 최적화)
    const CHUNK_SIZE = 1000;
    const results: ElevationElement[] = [];

    for (let i = 0; i < pointsToFetch.length; i += CHUNK_SIZE) {
      const chunk = pointsToFetch.slice(i, i + CHUNK_SIZE);
      const elevationData = await requestElevationBatch(
        chunk.map((p) => p.location),
      );
      results.push(...elevationData);
    }

    // 5. Mapping (Precision Lookup)
    // 좌표값의 부동소수점 오차를 방지하기 위해 string key로 매핑
    const resultMap = new Map<string, number>();
    results.forEach((res) => {
      const key = `${res.latitude.toFixed(6)},${res.longitude.toFixed(6)}`;
      resultMap.set(key, res.elevation ?? 0);
    });

    // 6. Final Assembly
    // 순서가 보장된 pointsToFetch 배열을 순회하며 고도값 매핑
    // 필터링 없이 Padding 영역 전체를 반환 (Repository 동작 일치)
    const finalPoints: Coord[] = pointsToFetch.map((p) => {
      const key = `${p.location.lat.toFixed(6)},${p.location.lng.toFixed(6)}`;
      const z = resultMap.get(key) ?? 0;
      return { x: p.pos.x, y: p.pos.y, z };
    });

    return finalPoints;
  });
}
