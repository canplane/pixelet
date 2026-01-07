/**
 * @file tile-fetcher.ts
 * [Bridge Layer] 서버의 DTO를 수신하여 런타임 Tile 인스턴스로 복원합니다.
 */

import type { Tile, TileDto, TileSurface } from '@pixelet/core/entities/geo/model/tiles';
import { TILE_SURFACE_KEYS } from '@pixelet/core/entities/geo/model/tiles';
import { OSM_CONFIG } from '@/entities/geo/config/osm-config';
import { buildExternalTile } from './tile-external-builder';
import type { TileWorkerInput } from '../../model/worker-protocol';
import { withSharedPromise } from '@pixelet/core/shared/lib/cache-utils';

// 🚀 New Shared Lib Codecs & Factory
import { JsonCodec } from '@pixelet/core/shared/lib/codec/json-codec';
import { SparseQuadtreeCodec } from '@pixelet/core/shared/lib/structs/sparse-quadtree-codec';
import { PayloadPacker } from '@pixelet/core/shared/lib/codec/payload-packer';
import { createEmptyTile } from '@pixelet/core/entities/geo/lib/utils/tile-factory';

import type { MapPoi } from '@pixelet/core/entities/geo/model/osm-feature';
import type { TileCoord } from '@pixelet/core/entities/geo/lib/utils/tile-utils';
import { SERVICE_MODES } from '@/config/app-config';
import { withRetry } from '@pixelet/core/shared/lib/aync-utils';

const { INTERNAL_GEO_ENDPOINT } = OSM_CONFIG;

/**
 * [Helper] LOCAL 모드: 서버에서 이미 베이킹이 완료된 타일 데이터를 획득 및 복원합니다.
 */
async function requestServerTile(coord: TileCoord): Promise<Tile> {
  const { tx, ty } = coord;
  const rawKey = `baked:${tx}:${ty}`;

  return withSharedPromise('local-tile', rawKey, async () => {
    return withRetry(
      async () => {
        const controller = new AbortController();
        const timeoutId = setTimeout(() => controller.abort(), 30000);

        try {
          const response = await fetch(INTERNAL_GEO_ENDPOINT, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ tx, ty }),
            signal: controller.signal,
          });

          clearTimeout(timeoutId);

          if (!response.ok) {
            throw new Error(`Local Tile Fetch Failed: HTTP ${response.status}`);
          }

          // 1. [Receive DTO]
          const dto: TileDto = await response.json();

          // ----------------------------------------------------------------------
          // 2. [Parallel Unpacking & Decoding]
          // 🚀 서버 통신 시 JSON 직렬화로 파괴된 바이너리 데이터를 복원하여 GPU 전송 효율을 극대화합니다.
          // ----------------------------------------------------------------------
          const [surfaceEntries, eleBin, poisBin] = await Promise.all([
            Promise.all(
              TILE_SURFACE_KEYS.map(async (key) => {
                const packed = dto.base.surface[key];
                const buffer = await PayloadPacker.unpack(packed);
                return [key, SparseQuadtreeCodec.decode(buffer, Uint8Array)] as const;
              }),
            ),
            PayloadPacker.unpack(dto.base.ele),
            PayloadPacker.unpack(dto.pois),
          ]);

          // 🚀 [Type-Safe Fix]
          // Object.fromEntries 대신 reduce를 사용하여 조립 과정의 타입 안정성을 확보합니다.
          const surface = surfaceEntries.reduce((acc, [key, tree]) => {
            acc[key] = tree;
            return acc;
          }, {} as Partial<TileSurface>) as TileSurface;

          const ele = SparseQuadtreeCodec.decode(eleBin, Int16Array);
          const pois = JsonCodec.decode<MapPoi[]>(poisBin) ?? [];

          // ----------------------------------------------------------------------
          // 3. [Instance Assembly]
          // [Memory Optimization] 새로운 Layered Tile 구조에 맞춰 정제하여 반환
          // 🚀 팩토리의 스켈레톤과 해체된 실데이터를 즉시 결합합니다.
          // ----------------------------------------------------------------------
          return {
            ...createEmptyTile(coord), // 🎯 뼈대 주입 (기본 필드 및 초기 상태 보장)
            key: dto.key,
            base: { surface, ele }, // 🎯 복원된 바이너리(SparseQuadtree) 주입
            pois,
            metadata: { ...dto.metadata }, // 🎯 서버측 버전 및 타임스탬프 동기화
          };
        } catch (error: any) {
          clearTimeout(timeoutId);
          if (error.name === 'AbortError') {
            throw new Error(`[Timeout] Local server response exceeded limit for tile ${tx}:${ty}`);
          }
          throw error;
        }
      },
      3,
      2000,
    );
  });
}

/**
 * [Main Entry] fetchTile
 * 워커로부터 타일 생성 요청을 받아 적절한 가공 전략을 실행하는 엔트리 포인트입니다.
 */
export async function fetchTile(coord: TileCoord, options: TileWorkerInput['options'], onProgress?: (partialTile: Tile) => void): Promise<Tile> {
  const { serviceMode } = options;

  /**
   * [Case A] Internal 모드
   * 우리 서버에 이미 구워진 완제품(Baked Tile)을 요청합니다.
   */
  if (serviceMode === SERVICE_MODES.INTERNAL) {
    return await requestServerTile(coord);
  }

  /**
   * [Case B] External 모드
   * Public API(OSM 등)를 통해 클라이언트에서 직접 지형을 베이킹합니다.
   */
  return await buildExternalTile(coord, onProgress);
}
