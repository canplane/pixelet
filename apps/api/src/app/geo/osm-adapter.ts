/**
 * @file osm-adapter.ts
 * DB의 Raw Row(GeoJSON, WKT, Hstore)를 시스템 표준 OsmInput 배열로 변환합니다.
 * 🚀 [Refactor] MultiPolygon을 개별 Polygon Feature로 분리(FlatMap)하여 반환합니다.
 */

import { Injectable } from '@nestjs/common';
import type { OsmInput } from '@pixelet/core/entities/osm/model/osm-feature';
import type { Coord } from '@pixelet/core/shared/model/coords';
import type { OsmTags } from '@pixelet/core/entities/osm/model/osm-source';

type GeoJsonCoord = [number, number];

/** 🎯 DB 엔티티 인터페이스 */
interface RawOsmRow {
  osm_id: string;
  tags: string | OsmTags | null;
  geometry: string;
  cwkt: string;
}

interface RawCoastlineRow {
  id: string;
  geometry: string;
}

@Injectable()
export class OsmAdapter {
  /**
   * [toOsmInput]
   * 일반 OSM 요소용
   * 🚀 Return Type Changed: OsmInput -> OsmInput[]
   */
  toOsmInput(raw: RawOsmRow): OsmInput[] {
    const tags = this.resolveTags(raw.tags);
    const geo = this.safeJsonParse(raw.geometry);
    const centerPoints = this.parseWktToCoord(raw.cwkt);
    // Center가 없으면 첫 번째 점을 사용하기 위해 나중에 계산
    const fallbackCenter = centerPoints[0];

    // 🚀 형상 파싱 결과를 배열로 받음
    const geometries = this.parseGeometry(geo);

    return geometries.map((geometry, index) => ({
      id: Number(raw.osm_id), // ID는 공유됩니다 (필요 시 index로 구분 가능)
      type: geometry.points.length === 1 ? 'node' : 'way',
      tags,
      geometry,
      // Center가 없으면 기하학적 첫 점을 사용
      center: fallbackCenter || geometry.points[0],
      isPolygonSource: geometry.isPolygonSource,
    }));
  }

  /**
   * [coastlineToOsmInput] 🌊
   * 해안선 전용
   * 🚀 Return Type Changed: OsmInput -> OsmInput[]
   */
  coastlineToOsmInput(raw: RawCoastlineRow): OsmInput[] {
    const geo = this.safeJsonParse(raw.geometry);

    // 🚀 MultiPolygon이 쪼개져서 geometries 배열로 반환됨
    const geometries = this.parseGeometry(geo);

    return geometries.map((geometry) => ({
      id: Number(raw.id),
      type: 'way',
      tags: {
        natural: 'ocean',
        source: 'osm-coastline',
      },
      geometry,
      center: geometry.points[0],
      isPolygonSource: true,
    }));
  }

  // --- [Internal Helpers] ---

  /**
   * 🎯 GeoJSON을 파싱하여 [1...N]개의 Geometry 구조체를 반환합니다.
   * 이제 MultiPolygon은 N개의 독립된 Geometry로 분리됩니다.
   */
  private parseGeometry(geo: any): Array<{
    points: Coord[];
    holes: Coord[][];
    isPolygonSource: boolean;
  }> {
    if (!geo) return [];

    switch (geo.type) {
      case 'Polygon': {
        const polyCoords = geo.coordinates as GeoJsonCoord[][];
        return [this.processPolygonRing(polyCoords, true)];
      }

      case 'MultiPolygon': {
        const multiCoords = geo.coordinates as GeoJsonCoord[][][];
        // 🚀 [FlatMap Strategy]
        // MultiPolygon의 각 요소를 '구멍'이 아닌 '독립된 폴리곤'으로 처리합니다.
        // 이렇게 해야 "섬(Island)"이 바다의 구멍(투명)이 아니라 진짜 육지 데이터로 살아남습니다.
        return multiCoords.map((polyCoords) =>
          this.processPolygonRing(polyCoords, true),
        );
      }

      case 'LineString': {
        const coords = geo.coordinates as GeoJsonCoord[];
        return [
          {
            points: this.coordsToMeter(coords),
            holes: [],
            isPolygonSource: false,
          },
        ];
      }

      case 'Point': {
        const [px, py] = geo.coordinates as GeoJsonCoord;
        return [
          {
            points: [{ x: px, y: py, z: 0 }],
            holes: [],
            isPolygonSource: false,
          },
        ];
      }

      default:
        return [];
    }
  }

  /**
   * [Helper] Polygon의 링(Ring) 구조를 points와 holes로 변환
   * @param rings GeoJSON 좌표 배열 (0번: Outer, 1~N번: Holes)
   */
  private processPolygonRing(rings: GeoJsonCoord[][], isPolygon: boolean) {
    const points = this.coordsToMeter(rings[0]);
    let holes: Coord[][] = [];

    // 0번째 링은 외곽선, 그 이후는 진짜 내부 구멍(Hole)
    if (rings.length > 1) {
      holes = rings.slice(1).map((ring) => this.coordsToMeter(ring));
    }

    return { points, holes, isPolygonSource: isPolygon };
  }

  /** 🎯 Tags 해석 (String Parsing vs Object) */
  private resolveTags(tagsRaw: any): OsmTags {
    if (typeof tagsRaw === 'string') {
      return this.parseHstore(tagsRaw) as OsmTags;
    }
    return (tagsRaw || {}) as OsmTags;
  }

  /** 🎯 좌표계 변환 및 Z-Axis 초기화 */
  private coordsToMeter(coords: GeoJsonCoord[]): Coord[] {
    return coords.map(([x, y]) => ({ x, y, z: 0 }));
  }

  /** 🎯 WKT(Point) 파싱 */
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

  /** 🎯 Hstore 파싱 (SSOT 유지) */
  private parseHstore(hstoreStr: string): Record<string, string> {
    const result: Record<string, string> = {};
    const pattern = /"(.+?)"=>"(.*?)"/g;
    let match;
    while ((match = pattern.exec(hstoreStr)) !== null) {
      result[match[1]] = match[2];
    }
    return result;
  }

  /** 🎯 안전한 JSON 파싱 (any 대신 unknown 에러 처리) */
  private safeJsonParse(jsonStr: string): any {
    try {
      return JSON.parse(jsonStr);
    } catch (error: unknown) {
      if (error instanceof Error) {
        // [Error Handling Policy] 파싱 실패 시 로그를 남기고 null 반환
        console.error(`[OsmAdapter] JSON Parse Error: ${error.message}`);
      }
      return null;
    }
  }
}
