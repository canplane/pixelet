/**
 * @file tile-processor.ts
 * 타일 데이터 획득 및 생애주기를 관리하는 메인 오케스트레이터입니다.
 * * [설계 의도]
 * 1. 환경(apiMode)에 따라 서버(Local) 완제품 소비 또는 클라이언트(Public) 직접 가공 전략을 선택합니다.
 * 2. 서버 통신 시 JSON 직렬화로 파괴된 바이너리 데이터(Uint8Array)를 복원하여 GPU 전송 효율을 극대화합니다.
 * 3. 클라이언트 측에서 origin을 재계산하여 서버-클라이언트 간의 미세한 좌표 오차를 방지합니다.
 */

import type {
  GeometryGrids,
  GeometryTile,
} from '@pixelet/shared/entities/osm/model/tiles';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import { OSM_CONFIG } from '@/entities/osm/config/osm-config';
import type { Coord } from '@pixelet/shared/shared/model/coords';
import { processPublicTile } from './tile-public-processor';
import type { TileWorkerInput } from '../../model/worker-protocol';
import { withSharedPromise } from '@pixelet/shared/shared/lib/cache-utils';

const { TILE_SIZE, CELL_SIZE } = OSM_CONSTANTS;
const { LOCAL_API_ENDPOINT } = OSM_CONFIG;

/**
 * [Helper] Base64 문자열을 8비트 배열로 복원
 * 서버에서 Buffer.from(arr).toString('base64')로 보낸 데이터를 디코딩합니다.
 */
const restore8 = (b64: any): Uint8Array => {
  // 만약 서버가 이미 배열로 보냈다면 그대로 사용 (호환성 유지)
  if (Array.isArray(b64) || b64 instanceof Uint8Array) {
    return new Uint8Array(b64);
  }
  // Base64 문자열 복원
  const binary = atob(b64 as string);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
};

/**
 * [Helper] Base64 문자열을 16비트 배열로 복원
 * 고도 데이터(height) 복원용
 */
const restore16 = (b64: any): Uint16Array => {
  if (Array.isArray(b64) || b64 instanceof Uint16Array) {
    return new Uint16Array(b64);
  }
  const binary = atob(b64 as string);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  // 바이트 배열을 16비트 뷰로 변환 (Endianness 주의: 보통 Little Endian)
  return new Uint16Array(bytes.buffer);
};

/**
 * [Helper] LOCAL 모드: 서버에서 이미 베이킹이 완료된 타일 데이터를 획득합니다.
 * @param coord - 타일 인덱스 {tx, ty}
 * @param origin - 타일의 좌상단 시작점 (Meter)
 */
async function requestBakedTile(
  coord: TileWorkerInput['coord'],
  origin: Coord,
): Promise<GeometryTile> {
  const { tx, ty } = coord;
  // 타일 당 그리드 해상도 (N x N)
  const resolution = Math.floor(TILE_SIZE / CELL_SIZE);

  const rawKey = `baked:${tx}:${ty}:${resolution}`;

  return withSharedPromise('local-tile', rawKey, async () => {
    const controller = new AbortController();

    // 🎯 서버 응답 지연 시 워커 스레드 점유 방지를 위한 30초 타임아웃
    const timeoutId = setTimeout(() => controller.abort(), 30000);

    try {
      const response = await fetch(LOCAL_API_ENDPOINT, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ tx, ty }),
        signal: controller.signal,
      });

      clearTimeout(timeoutId);

      if (!response.ok) {
        throw new Error(`Local Tile Fetch Failed: HTTP ${response.status}`);
      }

      // 서버로부터 받은 JSON 완제품
      // 데이터 구조: { grids: { layers: { base: "base64...", ... }, terrain: { height: "base64..." } } }
      const data = (await response.json()) as any;

      /**
       * 🚀 [성능 최적화: 계층형 바이너리 복원]
       * 서버의 Base64 압축 데이터를 고속으로 TypedArray로 변환합니다.
       * Array.from() 오버헤드를 완전히 제거했습니다.
       */
      const restoredGrids: GeometryGrids = {
        layers: {
          base: restore8(data.grids.layers.base),
          water: restore8(data.grids.layers.water),
          transport: restore8(data.grids.layers.transport),
          structure: restore8(data.grids.layers.structure),
        },
        terrain: {
          // 🏔️ 16비트 고도 데이터 복원
          height: restore16(data.grids.terrain.height),
        },
      };

      return {
        ...data,
        key: `${tx}:${ty}`,
        origin, // 🎯 [무결성 보장] 서버가 준 origin 대신 클라이언트 설정 기반 origin을 강제 주입하여 타일 정렬 오차 방지
        grids: restoredGrids,
        resolution,
      };
    } catch (error: any) {
      clearTimeout(timeoutId);
      if (error.name === 'AbortError') {
        throw new Error(
          `[Timeout] Local server response exceeded limit for tile ${tx}:${ty}`,
        );
      }
      // 상위 워커 핸들러에서 재시도 로직을 탈 수 있도록 에러 전파
      throw error;
    }
  });
}

/**
 * [Main Entry] fetchAndProcessTile
 * 워커로부터 타일 생성 요청을 받아 적절한 가공 전략을 실행하는 엔트리 포인트입니다.
 * * @param coord - 타일 좌표 정보
 * @param options - 렌더링 옵션 (apiMode 등)
 * @param onProgress - (Public 모드 전용) 단계별 가공 데이터 알림 콜백
 */
export async function fetchAndProcessTile(
  coord: TileWorkerInput['coord'],
  options: TileWorkerInput['options'],
  onProgress?: (partialTile: GeometryTile) => void,
): Promise<GeometryTile> {
  const { tx, ty } = coord;
  const { apiMode } = options;

  // 🎯 Floating Origin 및 렌더링 기준점이 되는 타일 절대 좌표 계산
  const origin: Coord = { x: tx * TILE_SIZE, y: ty * TILE_SIZE, z: 0 };

  /**
   * [Case A] LOCAL 모드
   * NestJS 백엔드에서 미리 베이킹(PostGIS + Baker)된 고속 완제품을 수신합니다.
   */
  if (apiMode === 'local') {
    return await requestBakedTile(coord, origin);
  }

  /**
   * [Case B] PUBLIC 모드
   * 클라이언트 워커가 Overpass API를 통해 실시간 Fetch -> Normalize -> Baking 과정을 직접 수행합니다.
   * 단계별(지형, 도로, 건물) 점진적 렌더링을 지원합니다.
   */
  return await processPublicTile(coord, origin, onProgress);
}
