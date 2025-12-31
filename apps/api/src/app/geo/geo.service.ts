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
  createEmptyGrids,
  getTileBbox,
  TileCoord,
} from '@pixelet/shared/entities/osm/lib/utils/tile-utils';
import { GridLayers } from '@pixelet/shared/entities/osm/model/tiles';
import { GeometryTileDto } from '@pixelet/shared/entities/osm/dto/tiles.dto';
import { normalizeOsmFeature } from '@pixelet/shared/entities/osm/lib/process/osm-feature-normalizer';
import { assembleTile } from '@pixelet/shared/entities/osm/lib/bake/tile-assembler';
import { bakeElevationGrid } from '@pixelet/shared/entities/terrain/lib/elevation-baker';
import { packTileGrids } from '@pixelet/shared/entities/osm/compression/tile-grid-packer';
import { packVector } from '@pixelet/shared/entities/osm/compression/vector-packer';

@Injectable()
export class GeoService {
  constructor(
    private readonly geoRepo: GeoRepository,
    private readonly osmAdapter: OsmAdapter,
  ) {}

  /**
   * [createGeometryTile]
   * 타일 좌표에 대해 바다(Sea), 육지(Land), 지형(Terrain)을 포함한 압축 데이터를 생성합니다.
   */
  async createGeometryTile(coord: TileCoord): Promise<GeometryTileDto> {
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
    const elevationGrid = bakeElevationGrid(elevationPoints);

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
    const { pois, partialLayers } = assembleTile(coord, allFeatures);

    // ----------------------------------------------------------------------
    // 6. [Assembly & Compression] 최종 조립 및 패킹
    // ----------------------------------------------------------------------
    const finalGrids = createEmptyGrids();

    finalGrids.terrain.elevation = elevationGrid;

    // sea 채널을 포함한 모든 partialLayers 병합
    // Terrain(16-bit) + Layers(8-bit)
    (Object.keys(partialLayers) as (keyof GridLayers)[]).forEach((channel) => {
      if (finalGrids.layers[channel] && partialLayers[channel]) {
        finalGrids.layers[channel].set(partialLayers[channel]);
      }
    });

    return {
      coord,
      // 🚀 Grids: 단일 바이너리 스트림으로 통합 패킹 (SoA + Gzip)
      grids: packTileGrids(finalGrids, gzipSync),
      // 📦 Vectors: JSON Stringify + Gzip (General Purpose)
      pois: packVector(pois, gzipSync),
    };
  }
}
