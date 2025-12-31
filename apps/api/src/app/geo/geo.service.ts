import { Injectable } from '@nestjs/common';
import { InjectRepository } from '@nestjs/typeorm';
import { Repository } from 'typeorm';
import { OsmElement } from '@/app/geo/entities/osm-element.entity';

import type { OsmFeature } from '@pixelet/shared/entities/osm/model/osm-feature';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import { Bbox, getTileBbox } from '@pixelet/shared/entities/osm/lib/tile-math';
import {
  createEmptyGrids,
  GeometryGrids,
  VoxelLayers,
  GeometryTile,
} from '@pixelet/shared/entities/osm/model/tiles';
import { parseOsmData } from '@pixelet/shared/entities/osm/lib/osm-parser';
import { bakeGeometryGrid } from '@pixelet/shared/entities/osm/lib/geometry-baker';
import { Coord } from '@pixelet/shared/shared/model/coords';
import { bakeElevationGrid } from '@pixelet/shared/entities/elevation/lib/elevation-baker';
import { normalizeOsmFeature } from '@pixelet/shared/entities/osm/lib/osm-feature-normalizer';
import { OsmInput } from '@pixelet/shared/entities/osm/model/osm-feature';

const { TILE_SIZE, CELL_SIZE, Z_SAMPLING_STEP } = OSM_CONSTANTS;

type GeoJsonCoord = [number, number];
@Injectable()
export class GeoService {
  constructor(
    @InjectRepository(OsmElement, 'geoConnection')
    private readonly geoRepo: Repository<OsmElement>,
  ) {}

  /**
   * [getProcessedTile]
   * 특정 타일 좌표(tx, ty)에 해당하는 지표면 형상(Geometry)과 엔티티(Entity)를 생성합니다.
   *
   * @param tx 타일 X 인덱스
   * @param ty 타일 Y 인덱스
   * @returns GeometryTile (Grids + POIs/Infras)
   */
  async getProcessedTile(tx: number, ty: number): Promise<GeometryTile> {
    // 1. Resolution Calculation
    // 물리적 타일 크기(4096m)를 셀 크기(2m)로 나눈 해상도 (2048 x 2048)
    const resolution = Math.floor(TILE_SIZE / CELL_SIZE);

    const key = `${tx}:${ty}`;
    const bbox = getTileBbox(tx, ty);

    // [Origin Definition] Floating Origin 대응을 위한 타일의 절대 기준점
    const origin = { x: tx * TILE_SIZE, y: ty * TILE_SIZE, z: 0 };

    // 2. [Parallel Execution] Data Fetching
    // - Elevation: SRTM 래스터 데이터 (Top-Down Order)
    // - OSM Map Data: 벡터 데이터 (Points, Lines, Polygons)
    const [elevationPoints, elements] = await Promise.all([
      this.getElevationGrid(bbox),
      this.getNearbyMapDataByTile(tx, ty),
    ]);

    // 3. [Terrain Baking] 16-bit Heightfield Generation
    // 희소 샘플링된(Sparse) 고도 점들을 보간(Interpolation)하여 전체 해상도의 높이 맵을 생성합니다.
    const heightGrid = bakeElevationGrid(elevationPoints);

    // 4. [Geometry Baking] 8-bit Surface Layers & Entity Extraction
    /**
     * 1. 파싱: 'osm-parser' 실행
     * 정규화된 데이터를 바탕으로 3D 엔진용 인프라(Way/Rel) 및 POI(Node) 객체를 생성합니다.
     * 이미 정규화가 끝났으므로 연산 오버헤드 없이 분류와 정렬만 수행합니다.
     */
    const { infras, pois } = parseOsmData(elements);

    /**
     * 2. 그리드 베이킹 (정규화된 데이터 직결)
     * 인프라 객체(infras) 대신, 더 원원 정보가 풍부한 normalizedElements를 베이커에 직접 전달하여
     * 래스터화(Rasterization) 정밀도를 높입니다.
     */
    let partialLayers: Partial<VoxelLayers> = {};
    if (elements.length > 0) {
      const result = bakeGeometryGrid(elements, origin);
      partialLayers = result.layers;
    }

    // 5. [Assembly] GeometryGrids 조립
    // 빈 그리드 컨테이너에 베이킹된 데이터를 채워넣습니다.
    const finalGrids = createEmptyGrids(resolution);

    // 🏔️ Terrain Channel (Heightmap)
    finalGrids.terrain.height = heightGrid;

    // 🗺️ Layer Channels (Attribute Maps)
    (Object.keys(partialLayers) as (keyof VoxelLayers)[]).forEach((channel) => {
      if (finalGrids.layers[channel] && partialLayers[channel]) {
        finalGrids.layers[channel].set(partialLayers[channel]);
      }
    });

    // 6. [Serialization] Performance Optimization
    // TypedArray를 JSON 배열로 변환하는 것은 CPU 비용이 매우 큽니다.
    // 이를 Buffer -> Base64 문자열로 직접 변환하여 직렬화 속도를 극대화하고 페이로드 크기를 줄입니다.
    const serializedGrids = {
      layers: {
        base: Buffer.from(finalGrids.layers.base).toString('base64'),
        water: Buffer.from(finalGrids.layers.water).toString('base64'),
        transport: Buffer.from(finalGrids.layers.transport).toString('base64'),
        structure: Buffer.from(finalGrids.layers.structure).toString('base64'),
      },
      terrain: {
        // Uint16Array는 내부 buffer 참조를 통해 변환
        height: Buffer.from(finalGrids.terrain.height.buffer).toString(
          'base64',
        ),
      },
    };

    return {
      key,
      origin,
      grids: serializedGrids as unknown as GeometryGrids, // Transferred as Base64 encoded object
      infras, // Local Coordinates (relative to origin)
      pois, // Local Coordinates (relative to origin)
      resolution,
    };
  }

  /**
   * [getElevationGrid]
   * PostGIS 래스터 쿼리를 통해 고도 데이터를 샘플링합니다.
   *
   * @strategy Union + Padding + Standard Order
   * 1. Padding: 타일 경계(Seam)의 보간 오차를 없애기 위해 상하좌우 1칸씩 더 넓게 조회합니다.
   * 2. Union: 여러 SRTM 파일에 걸쳐 있는 타일의 경우, 조각난 래스터를 병합합니다.
   * 3. Order: 이미지 표준인 Top-Down(북->남) 순서를 유지하여 Three.js 텍스처 좌표계와 정합성을 맞춥니다.
   */
  private async getElevationGrid(
    bbox: Bbox,
    step: number = Z_SAMPLING_STEP,
  ): Promise<Coord[]> {
    // 1. Calculate Grid Dimensions (Inner Tile)
    const innerWidth = Math.floor((bbox.maxX - bbox.minX) / step) + 1;
    const innerHeight = Math.floor((bbox.maxY - bbox.minY) / step) + 1;

    // 2. Apply Padding (1 cell margin)
    const pad = 1;
    const paddedWidth = innerWidth + pad * 2;
    const paddedHeight = innerHeight + pad * 2;

    const paddedMinX = bbox.minX - step * pad;
    const paddedMaxX = bbox.maxX + step * pad;
    const paddedMinY = bbox.minY - step * pad;
    const paddedMaxY = bbox.maxY + step * pad;

    // 3. PostGIS Query Execution
    const query = `
      WITH interest_area AS (
        SELECT ST_MakeEnvelope(
          ${paddedMinX}, ${paddedMinY}, ${paddedMaxX}, ${paddedMaxY}, 3857
        ) as geom
      ),
      clipped_chunks AS (
        SELECT ST_Clip(r.rast, i.geom) as rast
        FROM esa_elevation_raster r, interest_area i
        WHERE ST_Intersects(r.rast, i.geom)
      ),
      unioned_rast AS (
        -- [Merge] 경계선에 걸친 여러 래스터 조각을 하나로 합병
        SELECT ST_Union(rast) as rast
        FROM clipped_chunks
      ),
      resampled_rast AS (
        -- [Resample] 요청한 해상도(paddedWidth/Height)에 맞춰 픽셀 재배치
        SELECT ST_Resample(
          rast,
          ${paddedWidth}, ${paddedHeight},
          ${paddedMinX}, ${paddedMaxY},
          0, 0
        ) as rast
        FROM unioned_rast
      )
      -- [Extract] 2D 배열 형태의 값 추출
      SELECT (ST_DumpValues(rast)).valarray as z_values
      FROM resampled_rast;
    `;

    const result = await this.geoRepo.query(query);
    if (!result || result.length === 0 || !result[0].z_values) return [];

    const zValues: number[][] = result[0].z_values;
    const points: Coord[] = [];

    // 4. Data Extraction Loop (Top-Down / North -> South)
    // PostGIS 래스터 데이터는 일반적으로 북쪽(Top)이 Row 0입니다.
    // 이 순서를 그대로 유지하며, 패딩을 제외한 유효 타일 영역만 추출합니다.
    for (let row = pad; row < paddedHeight - pad; row++) {
      // worldY 계산: 북쪽(MaxY)에서 시작하여 아래로 감소
      const worldY = paddedMaxY - row * step;

      for (let col = pad; col < paddedWidth - pad; col++) {
        // worldX 계산: 서쪽(MinX)에서 시작하여 우측으로 증가
        const worldX = paddedMinX + col * step;
        const z = zValues[row]?.[col] ?? 0;

        // [Validation] 부동소수점 오차를 고려한 BBox 포함 여부 검사
        if (
          worldX >= bbox.minX - 0.1 &&
          worldX <= bbox.maxX + 0.1 &&
          worldY >= bbox.minY - 0.1 &&
          worldY <= bbox.maxY + 0.1
        ) {
          points.push({ x: worldX, y: worldY, z });
        }
      }
    }
    return points;
  }

  /**
   * [getNearbyMapDataByTile]
   * 해당 타일 영역(BBox)에 포함되거나 걸쳐있는 모든 OSM 요소를 조회합니다.
   * GeoJSON 형태로 변환 후 정규화(Normalization) 과정을 거칩니다.
   * Pipeline: DB Fetch -> Transform (Adapter) -> Normalize (Policy)
   */
  private async getNearbyMapDataByTile(
    tx: number,
    ty: number,
  ): Promise<OsmFeature[]> {
    const bbox = getTileBbox(tx, ty);
    const area3857 = `ST_MakeEnvelope(${bbox.minX}, ${bbox.minY}, ${bbox.maxX}, ${bbox.maxY}, 3857)`;

    // SQL 쿼리 로직 유지 (ST_SimplifyPreserveTopology 등)
    const raws: any[] = await this.geoRepo.query(`
      SELECT 
        osm_id, 
        tags, 
        ST_AsGeoJSON(ST_SimplifyPreserveTopology(way, 0.1)) as geometry, 
        ST_AsText(ST_Centroid(way)) as cwkt
      FROM osm_elements
      WHERE way && ${area3857} 
        OR ST_Intersects(way, ${area3857})
    `);

    // [Pipeline]
    // 1. Transform: DB Row -> Input Spec
    // 2. Normalize: Input Spec -> OsmFeature
    return raws
      .map((raw) => this.transformGeoJsonToInput(raw))
      .map((input) => normalizeOsmFeature(input));
  }

  /**
   * [transformGeoJsonToInput]
   * GeoJSON -> OsmNormalizationInput 변환 어댑터
   */
  private transformGeoJsonToInput(raw: any): OsmInput {
    // [Legacy Support] Hstore String Parsing
    const tags =
      typeof raw.tags === 'string'
        ? this.parseHstore(raw.tags)
        : raw.tags || {};

    const geo = JSON.parse(raw.geometry);

    let points: Coord[] = [];
    let holes: Coord[][] = [];
    let isPolygonSource = false;

    // [Parsing] GeoJSON Types Check
    if (geo.type === 'Polygon') {
      isPolygonSource = true;
      const coords = geo.coordinates as GeoJsonCoord[][];
      points = coords[0].map(([x, y]) => ({ x, y, z: 0 }));
      if (coords.length > 1) {
        holes = coords
          .slice(1)
          .map((ring) => ring.map(([x, y]) => ({ x, y, z: 0 })));
      }
    } else if (geo.type === 'MultiPolygon') {
      isPolygonSource = true;
      const coords = geo.coordinates as GeoJsonCoord[][][];
      // [Sorting] 가장 큰 폴리곤을 메인으로 (기존 로직 유지)
      const sortedPolygons = coords.sort((a, b) => b[0].length - a[0].length);
      const mainPolygon = sortedPolygons[0];

      points = mainPolygon[0].map(([x, y]) => ({ x, y, z: 0 }));

      // 홀 처리 로직 유지
      if (mainPolygon.length > 1) {
        holes = mainPolygon
          .slice(1)
          .map((ring) => ring.map(([x, y]) => ({ x, y, z: 0 })));
      }
      for (let i = 1; i < sortedPolygons.length; i++) {
        const poly = sortedPolygons[i];
        // 나머지 폴리곤도 일단 구멍 리스트에 추가 (단순화 처리)
        holes.push(poly[0].map(([x, y]) => ({ x, y, z: 0 })));
        if (poly.length > 1) {
          holes.push(
            ...poly.slice(1).map((r) => r.map(([x, y]) => ({ x, y, z: 0 }))),
          );
        }
      }
    } else if (geo.type === 'LineString') {
      const coords = geo.coordinates as GeoJsonCoord[];
      points = coords.map(([x, y]) => ({ x, y, z: 0 }));
    } else if (geo.type === 'Point') {
      const [x, y] = geo.coordinates as GeoJsonCoord;
      points = [{ x, y, z: 0 }];
    }

    // [Parsing] Center WKT
    const centerPoints = this.parseWktToCoord(raw.cwkt);
    const center = centerPoints[0];

    return {
      id: Number(raw.osm_id),
      type: points.length === 1 ? 'node' : 'way',
      tags,
      geometry: { points, holes },
      center,
      isPolygonSource,
    };
  }

  // ... (Helper Methods: parseWktToCoord, parseHstore 유지) ...
  private parseWktToCoord(wkt: string): Coord[] {
    if (!wkt) return [];
    const coordsMatch = wkt.match(/[\d\.-]+\s+[\d\.-]+/g);
    return coordsMatch
      ? coordsMatch.map((pair) => {
          const [x, y] = pair.split(/\s+/).map(Number);
          return { x, y, z: 0 };
        })
      : [];
  }

  private parseHstore(hstoreStr: string): Record<string, string> {
    const result: Record<string, string> = {};
    const pattern = /"(.+?)"=>"(.*?)"/g;
    let match;
    while ((match = pattern.exec(hstoreStr)) !== null) {
      result[match[1]] = match[2];
    }
    return result;
  }
}
