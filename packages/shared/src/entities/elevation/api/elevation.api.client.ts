import type { Location, MeterPos } from '@pixelet/shared/shared/model/coords';
import type { Bbox } from '@pixelet/shared/entities/osm/lib/tile-math';
import { projection } from '@pixelet/shared/shared/lib/map/projection';
import { withSharedPromise } from '@pixelet/shared/shared/lib/cache-utils';

interface ElevationElement {
  latitude: number;
  longitude: number;
  elevation: number;
}

interface ElevationApiResponse {
  results: ElevationElement[];
  status: string;
}

// 🎯 [Optimization]: 32 -> 128로 조정.
// 타일당 요청 수를 17회에서 1~2회로 줄여 브라우저 요청 한도를 방어합니다.
const SAMPLING_STEP = 128;
const { unproject } = projection;

/**
 * [Internal] 실제 API 서버에 대량의 좌표를 전달하여 고도를 응답받습니다.
 */
async function requestElevationBatch(
  points: Location[],
): Promise<ElevationElement[]> {
  const locations = points.map((p) => ({
    latitude: p.lat,
    longitude: p.lng,
  }));

  // 🎯 프록시 경로로 요청 (원래는 'https://api.open-elevation.com/api/v1/lookup')
  const response = await fetch('/elevation-proxy/api/v1/lookup', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
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
 * [requestElevationForTile]
 * 특정 미터 단위 BBox 범위 내에서 고도 데이터를 샘플링합니다.
 * SRTM 해상도(30m)를 고려하여 32m 간격으로 수집합니다.
 */
export async function requestElevation(bbox: Bbox): Promise<MeterPos[]> {
  // TILE_SIZE 4096, SAMPLING_STEP 128 -> count는 정확히 32
  const rawKey = `elevation:${bbox.minX},${bbox.minY}:${SAMPLING_STEP}`;

  return withSharedPromise('elevation', rawKey, async () => {
    const { minX, minY, maxX, maxY } = bbox;

    // 🎯 경계점을 포함하기 위해 (전체길이 / 간격) + 1 을 수행
    // 4096 / 128 + 1 = 33
    const xCount = Math.floor((maxX - minX) / SAMPLING_STEP) + 1;
    const yCount = Math.floor((maxY - minY) / SAMPLING_STEP) + 1;

    // 🎯 X축 좌표 (0, 128, ..., 4096)
    const lngs = Array.from({ length: xCount }, (_, j) => {
      const x = minX + j * SAMPLING_STEP;
      return { x, lng: unproject({ x, y: minY, z: 0 }).lng };
    });

    // 🎯 Y축 좌표 (0, 128, ..., 4096)
    const lats = Array.from({ length: yCount }, (_, i) => {
      const y = minY + i * SAMPLING_STEP;
      return { y, lat: unproject({ x: minX, y, z: 0 }).lat };
    });

    const points: {
      pos: Omit<MeterPos, 'z'>;
      location: Omit<Location, 'alt'>;
    }[] = [];
    for (const latObj of lats) {
      for (const lngObj of lngs) {
        points.push({
          pos: { x: lngObj.x, y: latObj.y },
          location: { lat: latObj.lat, lng: lngObj.lng },
        });
      }
    }

    // 3. Chunking & Fetch (기존 로직 유지)
    const CHUNK_SIZE = 1000;
    const results: ElevationElement[] = [];
    for (let i = 0; i < points.length; i += CHUNK_SIZE) {
      const chunk = points.slice(i, i + CHUNK_SIZE);
      const elevationData = await requestElevationBatch(
        chunk.map((p) => p.location),
      );
      results.push(...elevationData);
    }

    // 🎯 [Fix] 좌표 기반 Map 생성 (정합성 보장)
    // API 응답 데이터가 순서대로 오지 않을 경우를 대비합니다.
    const resultMap = new Map<string, number>();
    results.forEach((res) => {
      // 소수점 오차 방지를 위해 고정 소수점 문자열을 키로 사용
      const key = `${res.latitude.toFixed(6)},${res.longitude.toFixed(6)}`;
      resultMap.set(key, res.elevation ?? 0);
    });

    // 🎯 [Fix] 원본 points 순서에 맞춰 z값 매핑
    return points.map((p) => {
      const key = `${p.location.lat.toFixed(6)},${p.location.lng.toFixed(6)}`;
      const z = resultMap.get(key) ?? 0;

      // 만약 맵에서 못 찾았다면, 가장 가까운 results에서 찾거나 0으로 방어
      return { ...p.pos, z };
    });
  });
}
