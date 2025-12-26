// src/geo/geo.service.ts
import { Injectable } from '@nestjs/common';
import { InjectRepository } from '@nestjs/typeorm';
import { Repository } from 'typeorm';
import { OsmBuilding } from './entities/osm-building.entity';
import { OsmPoi } from './entities/osm-poi.entity';
import { OsmRoad } from './entities/osm-road.entity'; // 추가 필요

// ✨ Shared에서 이미 검증된 파서와 타입 가져오기
import type {
  OverpassElement,
  OverpassResponse,
} from '@pixelet/shared/entities/osm/api/overpass-types';
// import { parseOsmData } from '@pixelet/shared/entities/osm/lib/osm-parser';
// import { projection } from '@pixelet/shared/shared/lib/map/projection';

@Injectable()
export class GeoService {
  constructor(
    @InjectRepository(OsmBuilding, 'geoConnection') // 'geoConnection' 명시 필수
    private readonly buildingRepo: Repository<OsmBuilding>,
    @InjectRepository(OsmRoad, 'geoConnection')
    private readonly roadRepo: Repository<OsmRoad>,
    @InjectRepository(OsmPoi, 'geoConnection')
    private readonly poiRepo: Repository<OsmPoi>,
  ) {}

  // /**
  //  * 특정 좌표 주변의 인프라 및 POI를 가져와 MapInfra/MapPoi 규격으로 변환합니다.
  //  * Floating Origin 지침에 따라 서버/클라이언트 간 기준점 합의가 필요합니다.
  //  */
  // async getNearbyMapData(lat: number, lng: number, radius: number) {
  //   // 1. PostGIS를 이용한 반경 검색 (EPSG:3857 기준)
  //   const origin3857 = `ST_Transform(ST_SetSRID(ST_MakePoint(${lng}, ${lat}), 4326), 3857)`;

  //   const [rawBuildings, rawRoads, rawPois] = await Promise.all([
  //     this.buildingRepo
  //       .createQueryBuilder('b')
  //       .where(`ST_DWithin(b.geom, ${origin3857}, :radius)`, { radius })
  //       .getMany(),
  //     this.roadRepo
  //       .createQueryBuilder('r')
  //       .where(`ST_DWithin(r.geom, ${origin3857}, :radius)`, { radius })
  //       .getMany(),
  //     this.poiRepo
  //       .createQueryBuilder('p')
  //       .where(`ST_DWithin(p.geom, ${origin3857}, :radius)`, { radius })
  //       .getMany(),
  //   ]);

  //   // 2. DB 데이터를 OverpassElement[] 규격으로 가공 (Parser 호환용)
  //   // 파서가 필요한 최소한의 데이터(tags, nodes/geometry 등)를 채워줍니다.
  //   const elements: OverpassElement[] = [
  //     ...rawBuildings.map((b) => this.mapToElement(b, 'way')),
  //     ...rawRoads.map((r) => this.mapToElement(r, 'way')),
  //     ...rawPois.map((p) => this.mapToElement(p, 'node')),
  //   ];

  //   // 3. ✨ 드디어 등장! Shared의 parseOsmData로 최종 변환
  //   // projection.project 함수를 전달하여 좌표 변환까지 한 번에 처리
  //   const { infras, pois } = parseOsmData(elements, projection.project);

  //   return { infras, pois };
  // }

  async getNearbyMapData(
    lat: number,
    lng: number,
    radius: number,
  ): Promise<OverpassResponse> {
    // 1. PostGIS를 이용한 반경 검색 (EPSG:3857 기준 연산)
    // 연산 속도 최적화를 위해 DB 인덱스가 걸린 3857 좌표계에서 ST_DWithin 수행
    const origin3857 = `ST_Transform(ST_SetSRID(ST_MakePoint(${lng}, ${lat}), 4326), 3857)`;

    const [rawBuildings, rawRoads, rawPois] = await Promise.all([
      this.buildingRepo
        .createQueryBuilder('b')
        .select(['b.osm_id', 'b.name', 'b.building_type', 'b.levels'])
        // 전송 규격인 4326(위경도)으로 변환 후 파싱이 용이한 GeoJSON으로 추출
        .addSelect('ST_AsGeoJSON(ST_Transform(b.geom, 4326))', 'geom')
        .where(`ST_DWithin(b.geom, ${origin3857}, :radius)`, { radius })
        .getRawMany(),
      this.roadRepo
        .createQueryBuilder('r')
        .select(['r.osm_id', 'r.name', 'r.highway_type', 'r.oneway'])
        .addSelect('ST_AsGeoJSON(ST_Transform(r.geom, 4326))', 'geom')
        .where(`ST_DWithin(r.geom, ${origin3857}, :radius)`, { radius })
        .getRawMany(),
      this.poiRepo
        .createQueryBuilder('p')
        .select(['p.osm_id', 'p.name', 'p.poi_type'])
        .addSelect('ST_AsGeoJSON(ST_Transform(p.geom, 4326))', 'geom')
        .where(`ST_DWithin(p.geom, ${origin3857}, :radius)`, { radius })
        .getRawMany(),
    ]);

    // 2. DB 데이터를 OverpassElement[] 규격으로 가공 (Parser 호환용)
    // 파서가 필요한 최소한의 데이터(tags, nodes/geometry 등)를 채워줍니다.
    const elements: OverpassElement[] = [
      ...rawBuildings.map((b) => this.mapToElement(b, 'way', 'building')),
      ...rawRoads.map((r) => this.mapToElement(r, 'way', 'road')),
      ...rawPois.map((p) => this.mapToElement(p, 'node', 'poi')),
    ];

    // 3. [수정] 서버에서는 파싱하지 않습니다!
    // Raw 규격(OverpassResponse) 그대로 반환하여 클라이언트 파서에게 맡깁니다.
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

    // 1. osm_id를 우선 사용 (Number로 변환)
    const elementId = raw.osm_id
      ? Number(raw.osm_id)
      : raw.id
        ? Number(raw.id)
        : Math.floor(Math.random() * 1000000);

    return {
      type,
      id: elementId,

      // [보강] MapPoi 렌더링을 위해 center 좌표를 명시적으로 제공 (out center 시뮬레이션)
      center: type === 'node' ? geometry[0] : undefined,

      // [보강] MapInfra 3D 메쉬 생성을 위해 geometry 제공 (out geom 시뮬레이션)
      // PostGIS의 Geometry 데이터를 파서가 읽을 수 있는 좌표 배열로 변환하는 로직 필요
      // (일반적으로 파서는 nodes의 좌표를 캐시해서 사용함)
      geometry: geometry,

      tags: {
        // 2. name이 null이면 빈 문자열이나 'Building' 등으로 대체
        name: raw.name || '',

        ...(category === 'building' && {
          // DB의 building_type 컬럼 매핑
          building: raw.building_type || 'yes',
          // DB의 levels 컬럼 매핑 (null이면 '1'층으로 기본값 설정)
          'building:levels': raw.levels ? raw.levels.toString() : '1',
        }),

        ...(category === 'road' && {
          // 도로 테이블의 컬럼명도 이와 유사한지 확인이 필요합니다.
          highway: raw.highway_type || 'road',
          oneway: raw.oneway === 'yes' ? 'yes' : 'no',
        }),

        ...(category === 'poi' && {
          amenity: raw.poi_type,
        }),
        // 2025-12-26 지침: 전역 디자인 효과를 위해 필요한 추가 태그 주입
      },
    };
  }

  private convertGeomToOverpassGeometry(
    geomStr: string,
  ): { lat: number; lon: number }[] {
    if (!geomStr) return [];

    // ST_AsGeoJSON 결과물 파싱
    const geom = JSON.parse(geomStr);

    // 1. Point (POI)
    if (geom.type === 'Point') {
      const [lon, lat] = geom.coordinates;
      return [{ lat, lon }];
    }

    // 2. LineString (Road)
    if (geom.type === 'LineString') {
      return geom.coordinates.map(([lon, lat]: [number, number]) => ({
        lat,
        lon,
      }));
    }

    // 3. Polygon (Building)
    if (geom.type === 'Polygon') {
      // Polygon은 첫 번째 배열(외곽선)을 사용합니다.
      return geom.coordinates[0].map(([lon, lat]: [number, number]) => ({
        lat,
        lon,
      }));
    }

    return [];
  }
}
