/**
 * @file geo.service.ts
 * [Tile Generation Pipeline]
 * 타일 요청을 받아 Fetch(IO) -> Transform(CPU) -> Bake(Raster) -> Codec(Binary) -> Packer(Envelope) 과정을 조율합니다.
 */

import { Injectable } from '@nestjs/common';
// 💡 [Architecture] 플랫폼 중립적인 Shared Lib 도구들을 사용하여 서버-클라이언트 대칭성을 확보합니다.
import { JsonCodec } from '@pixelet/core/shared/lib/codec/json-codec';
import { SparseQuadtreeCodec } from '@pixelet/core/shared/lib/structs/sparse-quadtree-codec';
import { PayloadPacker } from '@pixelet/core/shared/lib/codec/payload-packer';

import { GeoRepository } from './geo.repository';
import { OsmAdapter } from './osm-adapter';

import { getTileBbox, getTileKey, TileCoord } from '@pixelet/core/entities/geo/lib/utils/tile-utils';
import { createEmptyTileDto } from '@pixelet/core/entities/geo/lib/utils/tile-factory';
import { normalizeOsmFeature } from '@pixelet/core/entities/geo/lib/process/osm-feature-normalizer';
import { assembleTile } from '@pixelet/core/entities/geo/lib/bake/tile-assembler';
import { bakeTileElevation } from '@pixelet/core/entities/geo/lib/bake/tile-elevation-baker';
import { TILE_SURFACE_KEYS, TileDto, TileSurfaceKey } from '@pixelet/core/entities/geo/model/tiles';

@Injectable()
export class GeoService {
  constructor(
    private readonly geoRepo: GeoRepository,
    private readonly osmAdapter: OsmAdapter,
  ) {}

  /**
   * [createTile]
   * 타일 좌표에 대해 지형(Elevation), 표면(Surface), 정보(POI)를 포함한 압축 데이터를 생성합니다.
   */
  async createTile(coord: TileCoord): Promise<TileDto> {
    // 💡 [System Timestamp] 이 트랜잭션의 기준 시점을 고정합니다.
    // I/O와 연산에 걸리는 시간과 관계없이 이 타일의 '생성 시점'은 단일값으로 유지됩니다.
    const timestamp = Date.now();

    const key = getTileKey(coord);
    const bbox = getTileBbox(coord);

    // ----------------------------------------------------------------------
    // 1. [Fetch] 병렬 데이터 조회 (I/O Bound)
    // 🚀 지형, 일반 지형지물, 해안선을 동시에 가져와 대기 시간을 최소화합니다.
    // ----------------------------------------------------------------------
    const [elevationPoints, rawElements, rawCoastlines] = await Promise.all([
      this.geoRepo.fetchElevationPoints(bbox),
      this.geoRepo.fetchRawMapData(bbox),
      this.geoRepo.fetchCoastlines(bbox),
    ]);

    // ----------------------------------------------------------------------
    // 2. [Elevation Processing] 고도 데이터 래스터화
    // 💡 원시 점 데이터를 SparseQuadtree(Int16) 구조로 굽습니다.
    // ----------------------------------------------------------------------
    const ele = bakeTileElevation(elevationPoints);

    // ----------------------------------------------------------------------
    // 3. [Feature Normalization] OSM 데이터 정규화
    // ----------------------------------------------------------------------
    const osmFeatures = rawElements.flatMap((raw) => this.osmAdapter.toOsmInput(raw)).map((input) => normalizeOsmFeature(input));

    // ----------------------------------------------------------------------
    // 4. [Coastline Processing] 해안선 특수 처리
    // 🌊 해안선 폴리곤은 쉐이더에서 바다(Ocean)로 판별하기 위한 플래그를 주입합니다.
    // ----------------------------------------------------------------------
    const oceanFeatures = rawCoastlines
      .flatMap((coast) => this.osmAdapter.coastlineToOsmInput(coast))
      .map((input) => {
        const feature = normalizeOsmFeature(input);
        feature.extra.isOcean = true;
        return feature;
      });

    // 🎯 합류: 모든 지형지물을 통합하여 어셈블러로 전송
    const allFeatures = [...oceanFeatures, ...osmFeatures];

    // ----------------------------------------------------------------------
    // 5. [Geometry Processing] 벡터 및 래스터 생성
    // 🚀 통합된 피처들을 통해 POI 리스트와 표면 채널(SparseQuadtree들)을 생성합니다.
    // ----------------------------------------------------------------------
    const { pois, surface } = assembleTile(coord, allFeatures);

    // ----------------------------------------------------------------------
    // 6. [Serialization & Packing] 최종 조립 및 패킹 (Critical Section)
    // 💡 [Principle] 데이터 성격에 따라 명시적 코덱을 사용하며, 최종 전송은 바이너리 패커가 담당합니다.
    // ----------------------------------------------------------------------

    // [6-1] Elevation Packing (ele: Int16Array 기반)
    const elePayload = await PayloadPacker.pack(SparseQuadtreeCodec.encode(ele));

    // [6-2] Surface Channels Packing (Uint8Array 기반)
    // 🚀 [SSOT] TILE_SURFACE_KEYS를 순회하며 자동으로 페이로드를 생성합니다.
    const surfacePayloads = Object.fromEntries(
      await Promise.all(TILE_SURFACE_KEYS.map(async (key) => [key, await PayloadPacker.pack(SparseQuadtreeCodec.encode(surface[key]))])),
    ) as Record<TileSurfaceKey, string>;

    // [6-3] POI Packing (MapPoi[] JSON 규격)
    const poisPayload = await PayloadPacker.pack(JsonCodec.encode(pois));

    // ----------------------------------------------------------------------
    // 7. [DTO Production] 클라이언트 응답용 객체 구성
    // ----------------------------------------------------------------------
    return {
      // 🎯 [DTO Protection] 기본 스키마(metadata 등)를 보장합니다.
      ...createEmptyTileDto(coord),
      key,
      // 🚀 [System Architecture] 레이어별로 명확히 분리된 페이로드 구조
      base: {
        surface: surfacePayloads,
        ele: elePayload,
      },
      pois: poisPayload,

      metadata: {
        ver: '1.0.0', // 전역 버전 관리 전략에 따라 주입 예정
        updatedAt: timestamp, // 🎯 최상단에서 고정한 타임스탬프 주입
      },
    };
  }
}
