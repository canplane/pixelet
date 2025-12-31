/**
 * @file osm-adapter.ts
 * DB의 Raw Row(GeoJSON, WKT, Hstore)를 시스템 표준 OsmInput으로 변환합니다.
 * 순수 데이터 변환 로직이므로 유닛 테스트가 용이합니다.
 */

import { Injectable } from '@nestjs/common';
import type { OsmInput } from '@pixelet/shared/entities/osm/model/osm-feature';
import type { Coord } from '@pixelet/shared/shared/model/coords';

type GeoJsonCoord = [number, number];

@Injectable()
export class OsmAdapter {
  /**
   * [toOsmInput]
   * DB에서 조회된 Raw 객체를 OsmNormalizationInput으로 변환합니다.
   */
  toOsmInput(raw: any): OsmInput {
    // [Legacy Support] Hstore String Parsing
    const tags =
      typeof raw.tags === 'string'
        ? this.parseHstore(raw.tags)
        : raw.tags || {};

    // Geometry Parsing (GeoJSON -> Internal Coords)
    const geo = JSON.parse(raw.geometry);
    const { points, holes, isPolygonSource } = this.parseGeometry(geo);

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

  // --- Internal Helpers ---

  /**
   * GeoJSON 타입별로 형상을 파싱하여 points와 holes를 추출합니다.
   */
  private parseGeometry(geo: any) {
    let points: Coord[] = [];
    let holes: Coord[][] = [];
    let isPolygonSource = false;

    // [Parsing] GeoJSON Types Check
    if (geo.type === 'Polygon') {
      isPolygonSource = true;
      const coords = geo.coordinates as GeoJsonCoord[][];
      points = this.coordsToMeter(coords[0]);
      if (coords.length > 1) {
        holes = coords.slice(1).map((ring) => this.coordsToMeter(ring));
      }
    } else if (geo.type === 'MultiPolygon') {
      isPolygonSource = true;
      const coords = geo.coordinates as GeoJsonCoord[][][];

      // [Sorting] 가장 큰 폴리곤을 메인으로 (기존 로직 유지)
      // 면적이 큰 순서대로 정렬하여 첫 번째를 외곽선으로 간주합니다.
      const sortedPolygons = coords.sort((a, b) => b[0].length - a[0].length);
      const mainPolygon = sortedPolygons[0];

      points = this.coordsToMeter(mainPolygon[0]);

      // 홀 처리 로직 유지
      if (mainPolygon.length > 1) {
        holes = mainPolygon.slice(1).map((ring) => this.coordsToMeter(ring));
      }
      // 나머지 폴리곤도 일단 구멍 리스트에 추가 (단순화 처리)
      for (let i = 1; i < sortedPolygons.length; i++) {
        const poly = sortedPolygons[i];
        holes.push(this.coordsToMeter(poly[0]));
        if (poly.length > 1) {
          holes.push(...poly.slice(1).map((r) => this.coordsToMeter(r)));
        }
      }
    } else if (geo.type === 'LineString') {
      const coords = geo.coordinates as GeoJsonCoord[];
      points = this.coordsToMeter(coords);
    } else if (geo.type === 'Point') {
      const [x, y] = geo.coordinates as GeoJsonCoord;
      points = [{ x, y, z: 0 }];
    }

    return { points, holes, isPolygonSource };
  }

  /** GeoJSON 좌표 배열을 시스템 표준 Coord 배열로 변환 */
  private coordsToMeter(coords: GeoJsonCoord[]): Coord[] {
    return coords.map(([x, y]) => ({ x, y, z: 0 }));
  }

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
