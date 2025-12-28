// src/geo/geo.service.ts
import { Injectable } from '@nestjs/common';
import { InjectRepository } from '@nestjs/typeorm';
import { Repository } from 'typeorm';
import { OsmBuilding } from './entities/osm-building.entity';
import { OsmPoi } from './entities/osm-poi.entity';
import { OsmRoad } from './entities/osm-road.entity';

import type {
  OverpassElement,
  OverpassResponse,
} from '@pixelet/shared/entities/osm/api/overpass-types';
import { OSM_CONFIG } from '@pixelet/shared/entities/osm/osm-config';
import { getTileBbox } from '@pixelet/shared/entities/osm/lib/tile-math';
import {
  createEmptyGrids,
  TileGrids,
  WorldTile,
} from '@pixelet/shared/entities/osm/model/tiles';
import { processOsmToGrids } from '@pixelet/shared/entities/osm/lib/tile-data-hander';
@Injectable()
export class GeoService {
  constructor(
    @InjectRepository(OsmBuilding, 'geoConnection')
    private readonly buildingRepo: Repository<OsmBuilding>,
    @InjectRepository(OsmRoad, 'geoConnection')
    private readonly roadRepo: Repository<OsmRoad>,
    @InjectRepository(OsmPoi, 'geoConnection')
    private readonly poiRepo: Repository<OsmPoi>,
  ) {}

  /**
   * [개선] 타일 인덱스를 직접 받아, 가공이 완료된 WorldTile을 반환합니다.
   */
  async getProcessedTile(tx: number, ty: number): Promise<WorldTile> {
    const { TILE_SIZE, CELL_SIZE } = OSM_CONFIG;
    const resolution = Math.floor(TILE_SIZE / CELL_SIZE);
    const key = `${tx}:${ty}`;

    console.log(`\n🚀 [Tile-Start] ${key}`);
    console.time(`Total-Process-${key}`);

    // 1. DB 데이터 조회 단계
    console.time(`Step1-DB-Query-${key}`);
    const overpassData = await this.getNearbyMapDataByTile(tx, ty);
    console.timeEnd(`Step1-DB-Query-${key}`);
    console.log(`   - Fetched elements: ${overpassData.elements.length}`);

    // 2. 베이킹 엔진 가동 단계
    const origin = { x: tx * TILE_SIZE, y: ty * TILE_SIZE, z: 0 };
    console.time(`Step2-Baking-Engine-${key}`);
    const { infras, pois, partialGrids } = processOsmToGrids(
      overpassData.elements,
      origin,
      { tileSize: TILE_SIZE, cellSize: CELL_SIZE },
    );
    console.timeEnd(`Step2-Baking-Engine-${key}`);

    // 3. 그리드 병합 및 메모리 복사 단계
    console.time(`Step3-Grid-Assembly-${key}`);
    const finalGrids = createEmptyGrids(resolution);
    (Object.keys(partialGrids) as (keyof TileGrids)[]).forEach((channel) => {
      if (partialGrids[channel]) {
        finalGrids[channel].set(partialGrids[channel]);
      }
    });
    console.timeEnd(`Step3-Grid-Assembly-${key}`);

    console.timeEnd(`Total-Process-${key}`);
    console.log(`🏁 [Tile-End] ${key}\n`);

    // 5. 최종 WorldTile 규격으로 반환
    return {
      key,
      origin,
      grids: finalGrids,
      infras,
      pois,
      resolution,
    };
  }

  /**
   * 위경도를 입력받아 해당 좌표가 속한 타일의 BBox를 계산하고,
   * 해당 범위 내의 모든 데이터를 가져옵니다.
   */
  private async getNearbyMapDataByTile(
    tx: number,
    ty: number,
  ): Promise<OverpassResponse> {
    // 2. 타일의 미터 범위(Bbox) 추출
    const bbox = getTileBbox(tx, ty);

    // 3. PostGIS 쿼리 최적화
    // ST_MakeEnvelope(minX, minZ, maxX, maxZ, srid)
    // DB 인덱스가 3857이라면 변환 오버헤드 없이 즉시 검색됩니다.
    const area3857 = `ST_MakeEnvelope(${bbox.minX}, ${bbox.minY}, ${bbox.maxX}, ${bbox.maxY}, 3857)`;

    // ✨ [최적화 2] 데이터 획득 병렬화 및 필요한 컬럼만 추출
    const [rawBuildings, rawRoads, rawPois] = await Promise.all([
      this.buildingRepo
        .createQueryBuilder('b')
        .select(['b.osm_id', 'b.building_type', 'b.levels', 'b.name'])
        // GeoJSON 변환은 DB 부하가 크므로, 꼭 필요한 경우에만 4326으로 변환합니다.
        .addSelect('ST_AsGeoJSON(ST_Transform(b.geom, 4326))', 'geom')
        .where(`ST_Intersects(b.geom, ${area3857})`)
        .getRawMany(),
      this.roadRepo
        .createQueryBuilder('r')
        .select(['r.osm_id', 'r.highway_type', 'r.name'])
        .addSelect('ST_AsGeoJSON(ST_Transform(r.geom, 4326))', 'geom')
        .where(`ST_Intersects(r.geom, ${area3857})`)
        .getRawMany(),
      this.poiRepo
        .createQueryBuilder('p')
        .select(['p.osm_id', 'p.poi_type', 'p.name'])
        .addSelect('ST_AsGeoJSON(ST_Transform(p.geom, 4326))', 'geom')
        .where(`ST_Intersects(p.geom, ${area3857})`)
        .getRawMany(),
    ]);

    // ✨ [최적화 3] 루프 내부 가공 로직 최적화
    const elements: OverpassElement[] = [];

    // Buildings
    for (const b of rawBuildings) {
      elements.push(this.mapToElement(b, 'way', 'building'));
    }
    // Roads
    for (const r of rawRoads) {
      elements.push(this.mapToElement(r, 'way', 'road'));
    }
    // POIs
    for (const p of rawPois) {
      elements.push(this.mapToElement(p, 'node', 'poi'));
    }

    return {
      version: 0.6,
      generator: 'Pixelet PostGIS Engine',
      osm3s: {
        timestamp_osm_base: new Date().toISOString(),
        copyright: 'OpenStreetMap contributors',
      },
      elements: elements,
    };
  }

  /**
   * TypeORM 엔티티(Raw)를 OverpassElement 규격으로 래핑합니다.
   */
  private mapToElement(
    raw: any,
    type: 'node' | 'way',
    category: string,
  ): OverpassElement {
    const geometry = this.convertGeomToOverpassGeometry(raw.geom);

    const elementId = raw.osm_id
      ? Number(raw.osm_id)
      : raw.id
        ? Number(raw.id)
        : Math.floor(Math.random() * 1000000);

    return {
      type,
      id: elementId,
      center: type === 'node' ? geometry[0] : undefined,
      geometry: geometry,
      tags: {
        name: raw.name || '',
        ...(category === 'building' && {
          building: raw.building_type || 'yes',
          'building:levels': raw.levels ? raw.levels.toString() : '1',
        }),
        ...(category === 'road' && {
          highway: raw.highway_type || 'road',
          oneway: raw.oneway === 'yes' ? 'yes' : 'no',
        }),
        ...(category === 'poi' && {
          amenity: raw.poi_type,
        }),
      },
    };
  }

  private convertGeomToOverpassGeometry(
    geomStr: string,
  ): { lat: number; lon: number }[] {
    if (!geomStr) return [];

    const geom = JSON.parse(geomStr);

    if (geom.type === 'Point') {
      const [lon, lat] = geom.coordinates;
      return [{ lat, lon }];
    }

    if (geom.type === 'LineString') {
      return geom.coordinates.map(([lon, lat]: [number, number]) => ({
        lat,
        lon,
      }));
    }

    if (geom.type === 'Polygon') {
      return geom.coordinates[0].map(([lon, lat]: [number, number]) => ({
        lat,
        lon,
      }));
    }

    return [];
  }
}
