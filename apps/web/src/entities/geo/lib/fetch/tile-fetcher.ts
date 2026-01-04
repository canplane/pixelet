/**
 * @file tile-processor.ts
 * 타일 데이터 획득 및 생애주기를 관리하는 메인 오케스트레이터입니다.
 * * [설계 의도]
 * 1. 환경(serviceMode)에 따라 서버(Local) 완제품 소비 또는 클라이언트(Public) 직접 가공 전략을 선택합니다.
 * 2. 서버 통신 시 JSON 직렬화로 파괴된 바이너리 데이터(Uint8Array)를 복원하여 GPU 전송 효율을 극대화합니다.
 */

import type { Tile } from '@pixelet/core/entities/geo/model/tiles';
import type { TileDto } from '@pixelet/core/entities/geo/dto/tiles.dto';
import { GEO_CONSTANTS } from '@pixelet/core/entities/geo/model/geo-constants';
import { OSM_CONFIG } from '@/entities/geo/config/osm-config';
import { buildExternalTile } from './tile-external-builder';
import type { TileWorkerInput } from '../../model/worker-protocol';
import { withSharedPromise } from '@pixelet/core/shared/lib/cache-utils';
import { unpackTileTerrain } from '@pixelet/core/entities/geo/compression/tile-terrain-packer';
import { unpackVector } from '@pixelet/core/entities/geo/compression/vector-packer';
import type { MapPoi } from '@pixelet/core/entities/geo/model/osm-feature';
import type { TileCoord } from '@pixelet/core/entities/geo/lib/utils/tile-utils';
import { SERVICE_MODES } from '@/config/app-config';

const { TILE_RES: res } = GEO_CONSTANTS;
const { INTERNAL_OSM_ENDPOINT } = OSM_CONFIG;

/**
 * [Helper] LOCAL 모드: 서버에서 이미 베이킹이 완료된 타일 데이터를 획득합니다.
 * @param coord - 타일 인덱스 {tx, ty}
 */
async function requestServerTile(coord: TileCoord): Promise<Tile> {
  const { tx, ty } = coord;

  const rawKey = `baked:${tx}:${ty}:${res}`;

  return withSharedPromise('local-tile', rawKey, async () => {
    const controller = new AbortController();

    // 🎯 서버 응답 지연 시 워커 스레드 점유 방지를 위한 30초 타임아웃
    const timeoutId = setTimeout(() => controller.abort(), 30000);

    try {
      const response = await fetch(INTERNAL_OSM_ENDPOINT, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ tx, ty }),
        signal: controller.signal,
      });

      clearTimeout(timeoutId);

      if (!response.ok) {
        throw new Error(`Local Tile Fetch Failed: HTTP ${response.status}`);
      }

      const dto: TileDto = await response.json(); // 🎯 DTO 수신

      // 🚀 병렬로 모든 압축 해제 및 객체 복원
      const [terrain, pois] = await Promise.all([
        unpackTileTerrain(dto.terrain, res),
        unpackVector<MapPoi>(dto.pois),
      ]);

      // [Memory Optimization] 필요한 필드만 정제하여 반환
      const { key, coord } = dto;
      return { key, coord, terrain, pois };
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
 * [Main Entry] fetchTile
 * 워커로부터 타일 생성 요청을 받아 적절한 가공 전략을 실행하는 엔트리 포인트입니다.
 * * @param coord - 타일 좌표 정보
 * @param options - 렌더링 옵션 (serviceMode 등)
 * @param onProgress - (Public 모드 전용) 단계별 가공 데이터 알림 콜백
 */
export async function fetchTile(
  coord: TileCoord,
  options: TileWorkerInput['options'],
  onProgress?: (partialTile: Tile) => void,
): Promise<Tile> {
  const { serviceMode } = options;

  /**
   * [Case A] internal 모드
   * 우리 서버에 이미 구워진 완제품(Baked Tile)을 요청합니다.
   */
  if (serviceMode === SERVICE_MODES.INTERNAL) {
    return await requestServerTile(coord);
  }

  /**
   * [Case B] external 모드 (serverless)
   * 서버 도움 없이 클라이언트가 직접 API를 긁어와서 빌드합니다.
   */
  return await buildExternalTile(coord, onProgress);
}
