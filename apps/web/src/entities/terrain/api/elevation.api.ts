import type { Coord4326, Coord } from '@pixelet/shared/shared/model/coords';
import type { Bbox } from '@pixelet/shared/entities/osm/lib/utils/tile-utils';
import { projection } from '@pixelet/shared/shared/lib/map/projection';
import { withSharedPromise } from '@pixelet/shared/shared/lib/cache-utils';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';

const { Z_SAMPLING_STEP } = OSM_CONSTANTS;

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
 * 서버의 'getElevationGrid'와 동일한 'Padding + Top-Down' 전략을 사용하여
 * 미터 단위 BBox 범위 내의 고도 데이터를 샘플링합니다.
 */
export async function requestElevation(
  bbox: Bbox,
  step: number = Z_SAMPLING_STEP,
): Promise<Coord[]> {
  const rawKey = `elevation:${bbox.minX},${bbox.minY}:${step}`;

  return withSharedPromise('elevation', rawKey, async () => {
    // 1. [Padding Strategy] 33x33(Inner) -> 35x35(Padded) 확장
    // 서버 로직과 동일하게 상하좌우 1칸씩 패딩을 확보하여 경계면 오차를 방지합니다.
    const innerWidth = Math.floor((bbox.maxX - bbox.minX) / step) + 1;
    const innerHeight = Math.floor((bbox.maxY - bbox.minY) / step) + 1;
    const pad = 1;

    const paddedWidth = innerWidth + pad * 2;
    const paddedHeight = innerHeight + pad * 2;

    const paddedMinX = bbox.minX - step * pad;
    // 🎯 [Correction] Top-Down 순회이므로 시작점(MaxY)을 Padding만큼 올려서 잡습니다.
    const paddedMaxY = bbox.maxY + step * pad;

    // 2. [Top-Down Order] 좌표 생성 (북 -> 남)
    // PostGIS 및 서버 로직과 동일하게 Row 0이 북쪽(MaxY)이 되도록 순회합니다.
    const pointsToFetch: { pos: Coord; location: Coord4326 }[] = [];

    for (let row = 0; row < paddedHeight; row++) {
      // Y축: 위(Max)에서 아래(Min)로 감소
      const worldY = paddedMaxY - row * step;

      for (let col = 0; col < paddedWidth; col++) {
        // X축: 좌(Min)에서 우(Max)로 증가
        const worldX = paddedMinX + col * step;

        const { lat, lng } = unproject({ x: worldX, y: worldY, z: 0 });
        pointsToFetch.push({
          pos: { x: worldX, y: worldY, z: 0 },
          location: { lat, lng },
        });
      }
    }

    // 3. Batch Request (API 호출)
    // 35x35 = 1225개 포인트를 청크 단위로 요청합니다.
    const CHUNK_SIZE = 1000;
    const results: ElevationElement[] = [];
    for (let i = 0; i < pointsToFetch.length; i += CHUNK_SIZE) {
      const chunk = pointsToFetch.slice(i, i + CHUNK_SIZE);
      const elevationData = await requestElevationBatch(
        chunk.map((p) => p.location),
      );
      results.push(...elevationData);
    }

    // 4. Mapping (O(1) Lookup)
    // API 응답 순서 보장을 위해 Map으로 매핑합니다.
    const resultMap = new Map<string, number>();
    results.forEach((res) => {
      const key = `${res.latitude.toFixed(6)},${res.longitude.toFixed(6)}`;
      resultMap.set(key, res.elevation ?? 0);
    });

    // 5. Filtering & Result Assembly
    // 패딩된 데이터를 모두 가져온 후, 실제 타일 영역(Inner)만 필터링하여 반환합니다.
    // (서버 코드의 `if (worldX >= bbox.minX ...)` 로직과 100% 일치)
    const finalPoints: Coord[] = [];

    for (const p of pointsToFetch) {
      const key = `${p.location.lat.toFixed(6)},${p.location.lng.toFixed(6)}`;
      const z = resultMap.get(key) ?? 0;

      // 부동소수점 오차를 고려한 범위 체크
      if (
        p.pos.x >= bbox.minX - 0.1 &&
        p.pos.x <= bbox.maxX + 0.1 &&
        p.pos.y >= bbox.minY - 0.1 &&
        p.pos.y <= bbox.maxY + 0.1
      ) {
        finalPoints.push({ x: p.pos.x, y: p.pos.y, z });
      }
    }

    return finalPoints;
  });
}
