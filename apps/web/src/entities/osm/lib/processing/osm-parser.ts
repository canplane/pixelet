/**
 * @file osm-parser.ts
 */

import type { OverpassElement, OverpassMember } from '../../model/types';
import type { Location, MeterPos } from '@/shared/types/map';

export interface ShapePath {
  points: MeterPos[];
  isClosed: boolean;
  holes?: MeterPos[][];
}

export interface MapInfra {
  id: string;
  type: 'building' | 'road' | 'water' | 'landuse';
  points: MeterPos[];
  center: MeterPos;
  shapePath?: ShapePath;
  height: number;
  tags: Record<string, string | undefined>;
}

export interface MapPoi {
  id: string;
  type: 'poi';
  category: string;
  name: string;
  center: MeterPos;
  tags: Record<string, string | undefined>;
}

export interface ParseResult {
  infras: MapInfra[];
  pois: MapPoi[];
}

/**
 * 레이어 렌더링 우선순위 (Baking 순서 결정)
 * 숫자가 낮을수록 밑바닥(Base), 높을수록 최상단(Structure)
 */
const LAYER_PRIORITY: Record<string, number> = {
  landuse: 1,
  water: 2,
  road: 3,
  building: 4,
} as const;

/**
 * [Advanced Parsing Logic]
 */
export const parseOsmData = (
  elements: OverpassElement[],
  project: (loc: Location) => MeterPos | null,
): ParseResult => {
  const result: ParseResult = { infras: [], pois: [] };
  if (!elements) return result;

  // 1. 중복 제거
  const elementMap: Map<number | string, OverpassElement> = new Map();
  elements.forEach((el: OverpassElement) => {
    if (el.id && !elementMap.has(el.id)) {
      elementMap.set(el.id, el);
    }
  });

  const uniqueElements: OverpassElement[] = Array.from(elementMap.values());

  uniqueElements.forEach((item: OverpassElement, idx: number): void => {
    const tags: Record<string, string | undefined> = item.tags || {};
    const rawLat: number | undefined = item.lat || item.center?.lat;
    const rawLon: number | undefined = item.lon || item.center?.lon;

    // [SECTION 1] POI 처리 (Node)
    if (item.type === 'node') {
      if (tags.name && rawLat && rawLon) {
        const poiPos = project({ lng: rawLon, lat: rawLat, alt: 0 });
        if (poiPos) {
          result.pois.push({
            id: `poi-${item.id || idx}`,
            type: 'poi',
            category:
              tags.amenity ||
              tags.shop ||
              tags.tourism ||
              tags.highway ||
              'place',
            name: tags.name,
            center: { ...poiPos },
            tags,
          });
        }
      }
      return;
    }

    // [SECTION 2] 인프라 처리 (Way & Relation)
    if (item.type === 'way' || item.type === 'relation') {
      const outerPoints: MeterPos[] = [];
      const innerHoles: MeterPos[][] = [];

      if (item.geometry) {
        item.geometry.forEach((pt) => {
          const projected = project({ lng: pt.lon, lat: pt.lat, alt: 0 });
          if (projected) outerPoints.push(projected);
        });
      } else if (item.type === 'relation' && item.members) {
        item.members.forEach((member) => {
          if (!member.geometry) return;
          const memberPoints = member.geometry
            .map((pt) => project({ lng: pt.lon, lat: pt.lat, alt: 0 }))
            .filter((v): v is MeterPos => v !== null);

          if (member.role === 'outer') outerPoints.push(...memberPoints);
          else if (member.role === 'inner') innerHoles.push(memberPoints);
        });
      }

      if (outerPoints.length < 2) return;

      // --- [Type Detection Update] ---
      const isBuilding = !!(
        tags.building ||
        tags['building:part'] ||
        tags.man_made === 'pier' ||
        tags.man_made === 'groyne' ||
        tags.man_made === 'embankment' ||
        tags.bridge === 'yes'
      );

      const isRoad = !!(
        tags.highway ||
        tags.railway ||
        tags['area:highway'] ||
        tags.junction
      );

      const isWater =
        tags.natural === 'water' ||
        tags.natural === 'cliff' ||
        tags.natural === 'coastline' ||
        !!tags.waterway ||
        tags.landuse === 'reservoir' ||
        tags.landuse === 'basin' ||
        tags.water === 'river';

      // ✨ [수정] Institutional, Parking, Sports 태그를 확실하게 포함
      const isLanduse = !!(
        tags.landuse ||
        tags.leisure ||
        tags.natural ||
        tags.amenity === 'parking' ||
        tags.amenity === 'university' ||
        tags.amenity === 'hospital' ||
        tags.amenity === 'school' ||
        tags.amenity === 'college' || // 추가
        tags.campus // 추가
      );

      if (!isBuilding && !isRoad && !isWater && !isLanduse) return;

      // [도로 너비 계산]
      if (isRoad) {
        const lanes = tags.lanes ? parseInt(tags.lanes) : 1;
        const widthTag = tags.width ? parseFloat(tags.width) : 0;
        const calculatedWidth = widthTag > 0 ? widthTag : lanes * 3.5;
        tags._calculatedWidth = calculatedWidth.toString();
      }

      // [Center 계산]
      let minX = Infinity,
        maxX = -Infinity,
        minZ = Infinity,
        maxZ = -Infinity;
      outerPoints.forEach((p) => {
        if (p.x < minX) minX = p.x;
        if (p.x > maxX) maxX = p.x;
        if (p.z < minZ) minZ = p.z;
        if (p.z > maxZ) maxZ = p.z;
      });

      const center: MeterPos = {
        x: (minX + maxX) * 0.5,
        y: 0,
        z: (minZ + maxZ) * 0.5,
      };

      // --- [Y-Offset Stacking Update] ---
      // ✨ 겹침 방지를 위한 미세 고도 조정 (단위: meter)
      // Base(0.02) < Institutional(0.025) < Parking(0.03) < Sports(0.035) < Road(0.05)
      const layerLevel = tags.layer ? parseFloat(tags.layer) : 0;

      const pointsWithOffset = outerPoints.map((pt) => {
        let y = 0;

        if (isRoad) {
          y = 0.05 + layerLevel * 0.2;
          if (tags.bridge === 'yes') y += 1.0;
          if (tags.tunnel === 'yes') y -= 1.0;
        } else if (isWater) {
          y = -0.15 + layerLevel * 0.1;
        } else if (isLanduse) {
          // 기본 지형
          y = 0.02;

          // [미세 조정]
          if (tags.landuse === 'forest' || tags.natural === 'wood') y = 0.03; // 나무는 살짝 위

          // Institutional (학교, 병원 캠퍼스)
          if (
            ['school', 'university', 'hospital', 'college'].includes(
              tags.amenity || '',
            )
          ) {
            y = 0.025;
          }

          // Parking (아스팔트니까 캠퍼스 잔디보다 위)
          if (tags.amenity === 'parking') {
            y = 0.03;
          }

          // Sports (운동장은 주차장/잔디보다 명확하게 위)
          if (['pitch', 'track', 'playground'].includes(tags.leisure || '')) {
            y = 0.035;
          }
        }
        return { ...pt, y };
      });

      // [건물 높이 계산]
      let infraHeight = 0;
      if (isBuilding) {
        const levels = tags['building:levels']
          ? parseFloat(tags['building:levels'])
          : 3;
        const minLevel = tags['building:min_level']
          ? parseFloat(tags['building:min_level'])
          : 0;
        const roofHeight = tags['roof:height']
          ? parseFloat(tags['roof:height'])
          : 0;
        const baseHeight = tags.height
          ? parseFloat(tags.height)
          : (levels - minLevel) * 3.5 + roofHeight;
        infraHeight = Math.max(0, baseHeight);
      }

      // [IsClosed 판정 Update]
      // 주차장(parking), 운동장(pitch)은 highway가 없어도 무조건 면(Polygon)으로 닫아야 함
      const forceClosed =
        tags.amenity === 'parking' ||
        ['pitch', 'track', 'playground', 'park', 'garden'].includes(
          tags.leisure || '',
        ) ||
        !!tags.landuse;

      const isClosed =
        item.type === 'relation' ||
        forceClosed || // ✨ 강제 폐곡선 처리
        (!isRoad && !tags.waterway && outerPoints.length >= 3) ||
        !!tags['area:highway'] ||
        (outerPoints.length > 2 &&
          outerPoints[0].x === outerPoints[outerPoints.length - 1].x &&
          outerPoints[0].z === outerPoints[outerPoints.length - 1].z);

      const shapePath: ShapePath = {
        points: pointsWithOffset,
        isClosed: isClosed,
        holes: innerHoles.length > 0 ? innerHoles : undefined,
      };

      const infraType = isBuilding
        ? 'building'
        : isRoad
          ? 'road'
          : isWater
            ? 'water'
            : 'landuse';

      result.infras.push({
        id: `infra-${item.id || idx}`,
        type: infraType,
        points: pointsWithOffset,
        center,
        shapePath,
        height: infraHeight,
        tags,
      });
    }
  });

  // 3. 최종 정렬
  result.infras.sort(
    (a, b) => (LAYER_PRIORITY[a.type] || 0) - (LAYER_PRIORITY[b.type] || 0),
  );

  return result;
};
