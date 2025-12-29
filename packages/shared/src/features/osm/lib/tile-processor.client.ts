/**
 * @file tile-processor.ts
 * 타일 데이터 획득 및 생애주기를 관리하는 메인 오케스트레이터입니다.
 * * [설계 의도]
 * 1. 환경(apiMode)에 따라 서버(Local) 완제품 소비 또는 클라이언트(Public) 직접 가공 전략을 선택합니다.
 * 2. 서버 통신 시 JSON 직렬화로 파괴된 바이너리 데이터(Uint8Array)를 복원하여 GPU 전송 효율을 극대화합니다.
 * 3. 클라이언트 측에서 origin을 재계산하여 서버-클라이언트 간의 미세한 좌표 오차를 방지합니다.
 */

import {
  type TileGrids,
  type WorldTile,
} from '@pixelet/shared/entities/osm/model/tiles';
import { OSM_CONFIG } from '@pixelet/shared/entities/osm/osm-config';
import type { MeterPos } from '@pixelet/shared/shared/model/coords';
import { processPublicTile } from './tile-public-processor.client';
import type { TileWorkerInput } from '@pixelet/shared/entities/osm/model/worker-types';
import { withSharedPromise } from '@pixelet/shared/shared/lib/cache-utils';

const { TILE_SIZE, CELL_SIZE, LOCAL_API_ENDPOINT } = OSM_CONFIG;

/**
 * [Helper] LOCAL 모드: 서버에서 이미 베이킹이 완료된 타일 데이터를 획득합니다.
 * @param coord - 타일 인덱스 {tx, ty}
 * @param origin - 타일의 좌상단 시작점 (Meter)
 */
async function requestBakedTile(
  coord: TileWorkerInput['coord'],
  origin: MeterPos,
): Promise<WorldTile> {
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

      const data = (await response.json()) as WorldTile;

      /**
       * [성능 최적화: 바이너리 복원 (Re-hydrating)]
       * JSON은 TypedArray(Uint8Array)를 지원하지 않아 전송 시 일반 number[]로 변환됩니다.
       * 이를 다시 Uint8Array로 래핑하여:
       * 1. WebGL 텍스처 데이터로 즉시 소비 가능하게 함
       * 2. 워커 -> 메인 스레드 전송 시 Zero-copy(Transferable Objects) 메커니즘 활성화
       */
      const restoredGrids: Partial<TileGrids> = {};
      const expectedSize = resolution * resolution;

      if (data?.grids) {
        for (const [key, val] of Object.entries(data.grids)) {
          if (Array.isArray(val)) {
            // 🚀 서버가 보낸 키(base, water 등)를 그대로 사용하여 Uint8Array 생성
            const arr = new Uint8Array(expectedSize);

            // 데이터가 있을 때만 채우고, 모자라면 0으로 유지됨 (Safety)
            // 서버 데이터가 부족하더라도 시스템이 죽지 않게 보호
            const sourceData = new Uint8Array(val);
            arr.set(sourceData.slice(0, expectedSize));

            // 🚀 데이터 복사 비용을 줄이기 위해 원본 배열을 TypedArray로 즉시 변환
            restoredGrids[key as keyof TileGrids] = arr;
          }
        }
      }

      // 🎯 [Safety Check]: 필수 채널인 height가 없으면 0으로라도 채워줌
      if (!restoredGrids.height) {
        restoredGrids.height = new Uint8Array(expectedSize).fill(0);
      }

      return {
        ...data,
        grids: restoredGrids as TileGrids, // 최종적으로 엔진 규격 타입으로 캐스팅
        key: `${tx}:${ty}`,
        // 🎯 [무결성 보장] 서버가 준 origin 대신 클라이언트 설정 기반 origin을 강제 주입하여 타일 정렬 오차 방지
        origin,
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
  onProgress?: (partialTile: WorldTile) => void,
): Promise<WorldTile> {
  const { tx, ty } = coord;
  const { apiMode } = options;

  // 🎯 Floating Origin 및 렌더링 기준점이 되는 타일 절대 좌표 계산
  const origin: MeterPos = { x: tx * TILE_SIZE, y: ty * TILE_SIZE, z: 0 };

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
