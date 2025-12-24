import { requestOverpass } from '../api/overpass.api';
import { MAP_QUERIES } from '../lib/overpass-queries';
import { parseOsmData } from '../lib/osm-parser';
import { bakeTileGrid } from '../lib/tile-baker';
import { OSM_CONFIG } from '../config/osm-config';
import type { Location, MeterPos } from '@/shared/types/map';
import type { WorldTile } from '@/features/map-three/model/map-store';

/**
 * [tile-processor]
 * 단일 타일의 생애주기(데이터 로드 -> 가공 -> 그리드 생성)를 책임지는 원자적 함수입니다.
 * 모든 결과 데이터는 서버 및 글로벌 기준과 동일한 '절대 미터 좌표계'를 유지합니다.
 * * @param tx - 타일 격자 X 인덱스
 * @param tz - 타일 격자 Z 인덱스
 * @param project - 위경도를 미터로 변환하는 함수
 * @param unproject - 미터를 위경도로 변환하는 함수
 * @param queryKeys - 실행할 쿼리 키 배열
 */
export async function fetchAndProcessTile(
  tx: number,
  tz: number,
  project: (loc: Location) => MeterPos | null,
  unproject: (pos: { x: number; y: number; z: number }) => Location,
  queryKeys: (keyof typeof MAP_QUERIES)[] = ['GEOMETRY_MAJOR', 'POI_MAJOR'],
): Promise<WorldTile> {
  const { TILE_SIZE, CELL_SIZE } = OSM_CONFIG;

  /** * origin: 타일의 시작점 (절대 미터 좌표)
   * MeterPos 인터페이스(x, y, z)를 충족하기 위해 y: 0을 명시합니다.
   */
  const origin: MeterPos = {
    x: tx * TILE_SIZE,
    y: 0,
    z: tz * TILE_SIZE,
  };
  const key = `${tx}:${tz}`;

  try {
    // 1. 타일 경계(BBox) 계산: Grid Index를 기반으로 위경도 범위를 도출합니다.
    const p1 = unproject({ x: origin.x, y: 0, z: origin.z });
    const p2 = unproject({
      x: origin.x + TILE_SIZE,
      y: 0,
      z: origin.z + TILE_SIZE,
    });

    const minLat = Math.min(p1.lat, p2.lat);
    const minLng = Math.min(p1.lng, p2.lng);
    const maxLat = Math.max(p1.lat, p2.lat);
    const maxLng = Math.max(p1.lng, p2.lng);
    const bbox = `${minLat},${minLng},${maxLat},${maxLng}`;

    // 2. Overpass API 호출: 정의된 쿼리 키에 따라 원본 OSM 데이터를 획득합니다.
    const requests = queryKeys.map((queryKey) => {
      const queryFn = MAP_QUERIES[queryKey] as any;
      return requestOverpass(queryFn({ bbox }));
    });

    const responses = await Promise.all(requests);
    const elements = responses.flatMap((res) => res.elements || []);

    /**
     * 3. OSM 데이터 파싱
     * raw 데이터를 절대 좌표(Meter) 기반의 인프라 및 POI 객체로 변환합니다.
     */
    const { infras, pois } = parseOsmData(elements, project);

    /**
     * 4. 타일 격자 데이터 생성 (Baking)
     * 절대 좌표 데이터를 기반으로 타일 규격에 맞는 Voxel Grid를 생성합니다.
     */
    const { grid, resolution } = bakeTileGrid(infras, origin, {
      tileSize: TILE_SIZE,
      cellSize: CELL_SIZE,
    });

    /**
     * 5. 가공 완료된 원자적 타일 데이터 반환 (WorldTile 인터페이스 준수)
     */
    return {
      key,
      origin,
      grid, // Voxel Type 데이터
      infras, // 절대 좌표 기반 벡터 데이터
      pois, // 절대 좌표 기반 관심 지점 데이터
      resolution, // 계산된 격자 해상도
    };
  } catch (error) {
    console.error(`[TileProcessor] Failed to process tile ${key}:`, error);
    throw error;
  }
}
