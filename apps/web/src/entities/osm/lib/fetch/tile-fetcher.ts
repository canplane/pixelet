/**
 * @file tile-processor.ts
 * 타일 데이터 획득 및 생애주기를 관리하는 메인 오케스트레이터입니다.
 * * [설계 의도]
 * 1. 환경(apiMode)에 따라 서버(Local) 완제품 소비 또는 클라이언트(Public) 직접 가공 전략을 선택합니다.
 * 2. 서버 통신 시 JSON 직렬화로 파괴된 바이너리 데이터(Uint8Array)를 복원하여 GPU 전송 효율을 극대화합니다.
 * 3. 클라이언트 측에서 origin을 재계산하여 서버-클라이언트 간의 미세한 좌표 오차를 방지합니다.
 */

import type { GeometryTile } from '@pixelet/shared/entities/osm/model/tiles';
import type { GeometryTileDto } from '@pixelet/shared/entities/osm/dto/tiles.dto';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import { OSM_CONFIG } from '@/entities/osm/config/osm-config';
import type { Coord } from '@pixelet/shared/shared/model/coords';
import { buildExternalTile } from './tile-external-builder';
import type { TileWorkerInput } from '../../model/worker-protocol';
import { withSharedPromise } from '@pixelet/shared/shared/lib/cache-utils';
import { unpackTileGrids } from '@pixelet/shared/entities/osm/compression/tile-grid-packer';
import { unpackVector } from '@pixelet/shared/entities/osm/compression/vector-packer';
import type { MapPoi } from '@pixelet/shared/entities/osm/model/osm-feature';
import type { TileCoord } from '@pixelet/shared/entities/osm/lib/utils/tile-utils.d';

const { TILE_SIZE, CELL_SIZE } = OSM_CONSTANTS;
const { INTERNAL_OSM_ENDPOINT } = OSM_CONFIG;

/**
 * [Helper] LOCAL 모드: 서버에서 이미 베이킹이 완료된 타일 데이터를 획득합니다.
 * @param coord - 타일 인덱스 {tx, ty}
 */
async function requestServerTile(coord: TileCoord): Promise<GeometryTile> {
  const { tx, ty } = coord;
  // 타일 당 그리드 해상도 (N x N)
  const resolution = Math.floor(TILE_SIZE / CELL_SIZE);

  const rawKey = `baked:${tx}:${ty}:${resolution}`;

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

      const dto: GeometryTileDto = await response.json(); // 🎯 DTO 수신

      // 🚀 병렬로 모든 압축 해제 및 객체 복원
      const [grids, pois] = await Promise.all([
        unpackTileGrids(dto.grids),
        unpackVector<MapPoi>(dto.pois),
      ]);

      return { ...dto, grids, pois };
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
 * @param options - 렌더링 옵션 (apiMode 등)
 * @param onProgress - (Public 모드 전용) 단계별 가공 데이터 알림 콜백
 */
export async function fetchTile(
  coord: TileCoord,
  options: TileWorkerInput['options'],
  onProgress?: (partialTile: GeometryTile) => void,
): Promise<GeometryTile> {
  const { tx, ty } = coord;
  const { serviceMode } = options;

  // 🎯 Floating Origin 및 렌더링 기준점이 되는 타일 절대 좌표 계산
  const origin: Coord = { x: tx * TILE_SIZE, y: ty * TILE_SIZE, z: 0 };

  /**
   * [Case A] internal 모드
   * 우리 서버에 이미 구워진 완제품(Baked Tile)을 요청합니다.
   */
  if (serviceMode === 'internal') {
    return await requestServerTile(coord);
  }

  /**
   * [Case B] external 모드 (serverless)
   * 서버 도움 없이 클라이언트가 직접 API를 긁어와서 빌드합니다.
   */
  return await buildExternalTile(coord, onProgress);
}
