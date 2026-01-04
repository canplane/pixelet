/**
 * @file geo.service.ts
 * [Tile Generation Pipeline]
 * 타일 요청을 받아 Fetch(IO) -> Transform(CPU) -> Bake(Raster) -> Compress(Binary) 과정을 조율합니다.
 */

import { Injectable } from '@nestjs/common';
import { gzipSync } from 'zlib';

import { GeoRepository } from './geo.repository';
import { OsmAdapter } from './osm-adapter';

import {
  createEmptyTerrain,
  getTileBbox,
  getTileKey,
  TileCoord,
} from '@pixelet/core/entities/osm/lib/utils/tile-utils';
import { TileChannels } from '@pixelet/core/entities/osm/model/tiles';
import { TileDto } from '@pixelet/core/entities/osm/dto/tiles.dto';
import { normalizeOsmFeature } from '@pixelet/core/entities/osm/lib/process/osm-feature-normalizer';
import { assembleTile } from '@pixelet/core/entities/osm/lib/bake/tile-assembler';
import { bakeTileElevation } from '@pixelet/core/entities/osm/lib/bake/tile-elevation-baker';
import { packTileTerrain } from '@pixelet/core/entities/osm/compression/tile-terrain-packer';
import { packVector } from '@pixelet/core/entities/osm/compression/vector-packer';

@Injectable()
export class GeoService {
  constructor(
    private readonly geoRepo: GeoRepository,
    private readonly osmAdapter: OsmAdapter,
  ) {}

  /**
   * [createTile]
   * 타일 좌표에 대해 바다(Sea), 육지(Land), 지형(Terrain)을 포함한 압축 데이터를 생성합니다.
   */
  async createTile(coord: TileCoord): Promise<TileDto> {
    const key = getTileKey(coord);
    const bbox = getTileBbox(coord);

    // ----------------------------------------------------------------------
    // 2. [Fetch] 병렬 데이터 조회 (I/O Bound)
    // ----------------------------------------------------------------------
    const [elevationPoints, rawElements, rawCoastlines] = await Promise.all([
      this.geoRepo.fetchElevationPoints(bbox),
      this.geoRepo.fetchRawMapData(bbox),
      this.geoRepo.fetchCoastlines(bbox),
    ]);

    // ----------------------------------------------------------------------
    // 3. [Terrain Baking] 16-bit 고도 맵 생성
    // ----------------------------------------------------------------------
    const elevationGrid = bakeTileElevation(elevationPoints);

    // ----------------------------------------------------------------------
    // 4. [Transformation] 데이터 정규화 파이프라인
    // ----------------------------------------------------------------------
    // [Flow]: OsmElement(DB) -> OsmInput(Adapter) -> OsmFeature(Domain)

    // 일반 OSM 요소 (건물, 도로 등)
    // 🚀 .flatMap 사용: toOsmInput이 배열을 반환하므로 평탄화 필요
    const osmFeatures = rawElements
      .flatMap((raw) => this.osmAdapter.toOsmInput(raw)) // DB 종속성 제거 및 규격화
      .map((input) => normalizeOsmFeature(input)); // 도메인 로직 적용 (좌표계, 타입 분류)

    // 🌊 바다 데이터 (Polygon 기반)
    // 이미 DB에서 EPSG:3857 Polygons로 정제되어 있으므로 변환이 매우 가볍습니다.
    // 🚀 .flatMap 사용: MultiPolygon이 쪼개진 배열을 평탄화
    const oceanFeatures = rawCoastlines
      .flatMap((coast) => this.osmAdapter.coastlineToOsmInput(coast)) // 전용 어댑터 메서드
      .map((input) => {
        const feature = normalizeOsmFeature(input);
        feature.extra.isOcean = true; // 쉐이더 판별용 플래그 주입
        return feature;
      });

    // 🎯 합류: 모든 지형지물을 통합하여 어셈블러로 전송 (이제 모든 폴리곤이 1:1로 펼쳐진 상태입니다)
    const allFeatures = [...oceanFeatures, ...osmFeatures];

    // ----------------------------------------------------------------------
    // 5. [Geometry Processing] 벡터 및 래스터 생성
    // ----------------------------------------------------------------------
    const { pois, partialChannels } = assembleTile(coord, allFeatures);

    // ----------------------------------------------------------------------
    // 6. [Assembly & Compression] 최종 조립 및 패킹
    // ----------------------------------------------------------------------
    const finalTerrain = createEmptyTerrain();

    finalTerrain.elevation = elevationGrid;

    // sea 채널을 포함한 모든 partialChannels 병합
    // Terrain(16-bit) + Channels(8-bit)
    (Object.keys(partialChannels) as (keyof TileChannels)[]).forEach(
      (channelKey) => {
        if (finalTerrain.channels[channelKey] && partialChannels[channelKey]) {
          finalTerrain.channels[channelKey].set(partialChannels[channelKey]);
        }
      },
    );

    return {
      key,
      coord,
      // 🚀 Grids: 단일 바이너리 스트림으로 통합 패킹 (SoA + Gzip)
      terrain: packTileTerrain(finalTerrain, gzipSync),
      // 📦 Vectors: JSON Stringify + Gzip (General Purpose)
      pois: packVector(pois, gzipSync),
    };
  }
}
