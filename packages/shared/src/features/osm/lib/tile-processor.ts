/**
 * @file tile-processor.ts
 * 타일 데이터 획득 및 생애주기를 관리하는 메인 오케스트레이터입니다.
 * 구성 환경(API 모드)에 따라 서버에서 완성된 데이터를 가져오거나,
 * 클라이언트 워커에서 직접 OSM 데이터를 가공(Baking)합니다.
 */

import {
  type TileGrids,
  type WorldTile,
} from '@pixelet/shared/entities/osm/model/tiles';
import { OSM_CONFIG } from '@pixelet/shared/entities/osm/osm-config';
import type { MeterPos } from '@pixelet/shared/shared/model/coords';
import { processPublicTile } from './tile-public-processor';
import type { TileWorkerInput } from '@pixelet/shared/entities/osm/model/worker-types';

const { TILE_SIZE, CELL_SIZE, LOCAL_API_ENDPOINT } = OSM_CONFIG;

/**
 * [Helper] LOCAL 모드: 서버에서 이미 베이킹이 완료된 타일 데이터를 획득합니다.
 * JSON 직렬화 과정에서 소실된 TypedArray(바이너리) 타입을 복원하여 성능을 최적화합니다.
 */
async function fetchBakedTile(
  coord: TileWorkerInput['coord'],
  origin: MeterPos,
  resolution: number,
): Promise<WorldTile> {
  const { tx, ty } = coord;
  const controller = new AbortController();

  const timeoutId = setTimeout(() => controller.abort(), 30000); // 10초 타임아웃 설정

  try {
    const response = await fetch(LOCAL_API_ENDPOINT, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ tx, ty }),
      signal: controller.signal,
    });

    clearTimeout(timeoutId);

    if (!response.ok) {
      throw new Error(
        `Local Tile Fetch Failed: ${response.status} ${response.statusText}`,
      );
    }

    const data = (await response.json()) as WorldTile;

    /**
     * [성능 최적화: 바이너리 복원]
     * JSON 통신 시 Uint8Array는 일반 number[]로 변환됩니다.
     * 이를 다시 Uint8Array로 래핑해야 워커 전송 시 Zero-copy(Transferable)가 작동합니다.
     */
    const restoredGrids: Partial<TileGrids> = {};
    if (data && data.grids) {
      Object.entries(data.grids).forEach(([key, val]) => {
        // JSON으로 넘어온 값은 number[] 타입이므로 Uint8Array로 복원
        restoredGrids[key as keyof TileGrids] = new Uint8Array(
          val as unknown as number[],
        );
      });
    }

    return {
      ...data,
      grids: restoredGrids as TileGrids, // 최종 캐스팅
      key: `${tx}:${ty}`,
      origin,
      resolution,
    };
  } catch (error: any) {
    clearTimeout(timeoutId);
    if (error.name === 'AbortError') {
      throw new Error(
        `[Timeout] Local server response exceeded limit for ${tx}:${ty}`,
      );
    }
    console.error(`[TileProcessor] Local fetch error:`, error);
    throw error; // 에러를 상위 워커 핸들러로 전파
  }
}

/**
 * [Main Entry] fetchAndProcessTile
 * 워커로부터 구조화된 데이터를 주입받아 타일 생성 전략을 실행합니다.
 * * @param coord - 타일 인덱스 정보 { tx, ty }
 * @param options - 실행 옵션 { apiMode, cellSize, ... }
 * @param onProgress - 점진적 업데이트 콜백
 */
export async function fetchAndProcessTile(
  coord: TileWorkerInput['coord'],
  options: TileWorkerInput['options'],
  onProgress?: (partialTile: WorldTile) => void,
): Promise<WorldTile> {
  const { tx, ty } = coord;
  const { apiMode } = options;

  const resolution = Math.floor(TILE_SIZE / CELL_SIZE);

  // 1. 타일별 절대 미터 좌표계 기준점(Origin) 설정
  const origin: MeterPos = {
    x: tx * TILE_SIZE,
    y: ty * TILE_SIZE,
    z: 0,
  };

  /**
   * [Case A] LOCAL 모드
   * 서버(NestJS)에서 모든 가공이 끝난 WorldTile 객체를 즉시 가져옵니다.
   */
  if (apiMode === 'local') {
    return await fetchBakedTile(coord, origin, resolution);
  }

  /**
   * [Case B] PUBLIC 모드
   * 클라이언트가 직접 Overpass API를 호출하고 단계적으로 그리드를 생성(Baking)합니다.
   */
  return await processPublicTile(coord, origin, resolution, onProgress);
}
