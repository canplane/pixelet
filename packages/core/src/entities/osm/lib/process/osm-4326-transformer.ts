/**
 * @file osm-4326-transformer.ts
 * @description Raw OSM API(4326) 데이터를 엔진 표준인 3857 Input 규격으로 변환합니다.
 */
import type { Osm4326Coordinate, Osm4326Input } from '../../model/osm-source';
import type { OsmInput } from '@pixelet/core/entities/osm/model/osm-feature';
import { projection } from '@pixelet/core/shared/lib/map/projection';
import type { Coord } from '@pixelet/core/shared/model/coords';

const { project } = projection;

/**
 * [Transformer] transformOsm4326
 * 위경도 좌표계를 미터 좌표계로 투영하고, 정규화를 위한 중간 객체를 생성합니다.
 */
export const transformOsm4326 = (elements: Osm4326Input[]): OsmInput[] => {
  if (!elements || elements.length === 0) return [];

  const nodeCache = new Map<number, Coord>();

  // 1. [Projection] Node Caching (4326 -> 3857)
  elements.forEach((el) => {
    if (el.type === 'node' && el.lat !== undefined && el.lon !== undefined) {
      const projected = project({ lat: el.lat, lng: el.lon, alt: 0 });
      if (projected) nodeCache.set(el.id, projected);
    }
  });

  return elements
    .map((item, idx): OsmInput | null => {
      // 2. [Center]
      const rawLat = item.lat || item.center?.lat;
      const rawLon = item.lon || item.center?.lon;
      const center =
        rawLat !== undefined && rawLon !== undefined
          ? project({ lat: rawLat, lng: rawLon, alt: 0 })
          : undefined;

      // 3. [Geometry Projection]
      const resolvePoints = (
        geom?: Osm4326Coordinate[],
        nodes?: number[],
      ): Coord[] => {
        if (geom?.length) {
          return geom
            .map((pt) => project({ lng: pt.lon, lat: pt.lat, alt: 0 }))
            .filter((v): v is Coord => v !== null);
        }
        if (nodes?.length) {
          return nodes
            .map((id) => nodeCache.get(id))
            .filter((v): v is Coord => v !== undefined) as Coord[];
        }
        return [];
      };

      let points: Coord[] = [];
      const holes: Coord[][] = [];

      if (item.type === 'way') {
        points = resolvePoints(item.geometry, item.nodes);
      } else if (item.type === 'relation' && item.members) {
        item.members.forEach((member) => {
          const mPoints = resolvePoints(member.geometry, member.nodes);
          if (mPoints.length < 2) return;
          if (member.role === 'inner') holes.push(mPoints);
          else points.push(...mPoints);
        });
      }

      if (item.type !== 'node' && points.length < 2) return null;

      // 4. [Output] 중간 객체 생성 (도메인 로직 제외)
      return {
        id: item.id || idx,
        type: item.type === 'relation' ? 'way' : item.type,
        tags: item.tags || {},
        geometry: { points, holes },
        center,
        // Relation은 소스 차원에서 닫힌 구조를 지향하므로 힌트 제공
        isPolygonSource: item.type === 'relation',
      };
    })
    .filter((v): v is OsmInput => v !== null);
};
