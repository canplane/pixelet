/**
 * @file geo.service.ts
 * [Tile Generation Pipeline]
 * 타일 요청을 받아 Fetch(IO) -> Transform(CPU) -> Bake(Raster) -> Compress(Binary) 과정을 조율합니다.
 */

import { Injectable } from '@nestjs/common';
import { gzipSync } from 'zlib';

import { GeoRepository } from './geo.repository';
import { OsmAdapter } from './osm-adapter';

import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
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
   * [getProcessedTile]
   * 클라이언트가 요청한 타일 좌표(tx, ty)에 대해 렌더링 가능한 압축 데이터(DTO)를 생성합니다.
   *
   * @param tx 타일 X 인덱스
   * @param ty 타일 Y 인덱스
   * @returns GeometryTileDto (Packed Binary Grids + Compressed Vector Data)
   */
  async getProcessedTile(coord: TileCoord): Promise<GeometryTileDto> {
    const { tx, ty } = coord;

    // ----------------------------------------------------------------------
    // 1. [Context Setup] 타일 공간 정의
    // ----------------------------------------------------------------------
    const bbox = getTileBbox(coord);

    // ----------------------------------------------------------------------
    // 2. [Fetch] 병렬 데이터 조회 (I/O Bound)
    // ----------------------------------------------------------------------
    // - elevationPoints: DEM(Digital Elevation Model) 포인트
    // - rawElements: DB에서 갓 나온 순수 엔티티 (OsmElement[])
    const [elevationPoints, rawElements] = await Promise.all([
      this.geoRepo.fetchElevationPoints(bbox),
      this.geoRepo.fetchRawMapData(bbox),
    ]);

    // ----------------------------------------------------------------------
    // 3. [Terrain Baking] 지형 생성 (16-bit)
    // ----------------------------------------------------------------------
    // 희소 샘플링된 고도 점들을 보간(Interpolation)하여 전체 해상도의 HeightMap 생성
    const heightGrid = bakeElevationGrid(elevationPoints);

    // ----------------------------------------------------------------------
    // 4. [Transformation] 데이터 정규화 파이프라인
    // ----------------------------------------------------------------------
    // [Flow]: OsmElement(DB) -> OsmInput(Adapter) -> OsmFeature(Domain)
    const elements = rawElements
      .map((raw) => this.osmAdapter.toOsmInput(raw)) // DB 종속성 제거 및 규격화
      .map((input) => normalizeOsmFeature(input)); // 도메인 로직 적용 (좌표계, 타입 분류)

    // ----------------------------------------------------------------------
    // 5. [Geometry Processing] 벡터 및 래스터 생성
    // ----------------------------------------------------------------------
    const { pois, partialLayers } = assembleTile(coord, elements);

    // ----------------------------------------------------------------------
    // 6. [Assembly & Compression] 최종 조립 및 패킹
    // ----------------------------------------------------------------------
    const finalGrids = createEmptyGrids();

    // Terrain(16-bit) + Layers(8-bit) 병합
    finalGrids.terrain.height = heightGrid;
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
