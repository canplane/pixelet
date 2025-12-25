/**
 * @file osm-parser.ts
 * Raw OSM 데이터를 엔진에서 사용 가능한 인프라 및 POI 객체로 변환합니다.
 * 레이어 간의 고도(Y-Offset)를 조정하여 Z-Fighting을 방지하고 렌더링 우선순위를 결정합니다.
 */

import type { OverpassElement } from '../../model/types';
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
 * Overpass API 응답을 파싱하여 3D 엔진용 객체로 변환합니다.
 * * * [BBox & Clipping Policy]
 * - (._; > ;) 전략으로 인해 현재 타일(BBox) 경계에 걸친 인프라의 경우 타일 외부 좌표가 유입됩니다.
 * - 본 파서는 데이터 무결성을 위해 외부 점을 강제로 절단(Clipping)하지 않고 온전히 보존합니다.
 * - 타일 간의 매끄러운 연결(Seamless joining)을 위해 인프라의 기하 구조를 원본 그대로 유지하며,
 * 실제 가시 영역 제한은 이후 Baking(Rasterization) 또는 렌더링 단계에서 처리합니다.
 */
export const parseOsmData = (
  elements: OverpassElement[],
  project: (loc: Location) => MeterPos | null,
): ParseResult => {
  const result: ParseResult = { infras: [], pois: [] };
  if (!elements || elements.length === 0) return result;

  /**
   * [보강: PASS 1] 모든 노드 좌표 사전 구축
   * Way/Relation이 geometry 없이 nodes ID 리스트만 가질 경우 좌표를 복구하기 위한 인덱스입니다.
   * [BBox Outside] 타일 외부에 위치한 참조 노드들도 누락 없이 캐싱하여 객체 복원율을 100%로 유지합니다.
   */
  const nodeCache = new Map<number, MeterPos>();
  elements.forEach((el) => {
    if (el.type === 'node' && el.lat !== undefined && el.lon !== undefined) {
      const projected = project({ lat: el.lat, lng: el.lon, alt: 0 });
      if (projected) nodeCache.set(el.id, projected);
    }
  });

  // 1. 중복 제거 (API 응답 내 동일 ID 객체 방어)
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

    // [SECTION 1] POI 처리 (Node 기반 장소 정보)
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

    // [SECTION 2] 인프라 처리 (Way & Relation 기반 지형/구조물)
    if (item.type === 'way' || item.type === 'relation') {
      const outerPoints: MeterPos[] = [];
      const innerHoles: MeterPos[][] = [];

      /**
       * [보강] 좌표 추출 통합 헬퍼
       * 1. geometry 배열이 있으면 우선 사용 (서버 계산값)
       * 2. 없으면 nodes ID 리스트를 nodeCache에서 참조하여 복구
       */
      const resolvePoints = (geom?: any[], nodes?: number[]): MeterPos[] => {
        if (geom && geom.length > 0) {
          return geom
            .map((pt) => project({ lng: pt.lon, lat: pt.lat, alt: 0 }))
            .filter((v): v is MeterPos => v !== null);
        }
        if (nodes && nodes.length > 0) {
          return nodes
            .map((id) => nodeCache.get(id))
            .filter((v): v is MeterPos => v !== undefined) as MeterPos[];
        }
        return [];
      };

      // Geometry 추출 (노드 복구 로직 포함)
      if (item.type === 'way') {
        outerPoints.push(...resolvePoints(item.geometry, item.nodes));
      } else if (item.type === 'relation' && item.members) {
        item.members.forEach((member) => {
          const memberPoints = resolvePoints(member.geometry, member.nodes);
          if (memberPoints.length < 2) return;

          if (member.role === 'inner') innerHoles.push(memberPoints);
          else outerPoints.push(...memberPoints);
        });
      }

      if (outerPoints.length < 2) return;

      // --- [Type Detection & Optimization] ---
      const isBuilding = !!(
        tags.building ||
        tags['building:part'] ||
        tags.man_made === 'pier' ||
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
        !!tags.waterway ||
        tags.landuse === 'reservoir' ||
        tags.water === 'river';

      const isInstitutional =
        ['school', 'university', 'hospital', 'college'].includes(
          tags.amenity || '',
        ) || !!tags.campus;
      const isParking =
        tags.amenity === 'parking' || tags.parking === 'surface';
      const isSports = ['pitch', 'track', 'playground', 'stadium'].includes(
        tags.leisure || '',
      );

      const isLanduse = !!(
        tags.landuse ||
        tags.leisure ||
        tags.natural ||
        isInstitutional ||
        isParking ||
        isSports
      );

      if (!isBuilding && !isRoad && !isWater && !isLanduse) return;

      // 내부 플래그 주입
      if (isInstitutional) tags._isInstitutional = 'true';
      if (isParking) tags._isParking = 'true';
      if (isSports) tags._isSports = 'true';

      if (isRoad) {
        const lanes = tags.lanes ? parseInt(tags.lanes) : 1;
        const widthTag = tags.width ? parseFloat(tags.width) : 0;
        const calculatedWidth = widthTag > 0 ? widthTag : lanes * 3.5;
        tags._calculatedWidth = calculatedWidth.toString();
      }

      // [Center 계산] 바운딩 박스 기반 중심점 도출
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

      // --- [Y-Offset Stacking & Z-Fighting Defense] ---
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
          y = 0.02;
          if (isInstitutional) y = 0.025;
          if (isParking) y = 0.03;
          if (isSports) y = 0.035;
          if (tags.landuse === 'forest' || tags.natural === 'wood') y = 0.028;
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

      // [IsClosed 판정]
      const forceClosed =
        isParking ||
        isSports ||
        isInstitutional ||
        ['park', 'garden'].includes(tags.leisure || '') ||
        !!tags.landuse;

      const isClosed =
        item.type === 'relation' ||
        forceClosed ||
        (!isRoad && !tags.waterway && outerPoints.length >= 3) ||
        !!tags['area:highway'] ||
        (outerPoints.length > 2 &&
          Math.abs(outerPoints[0].x - outerPoints[outerPoints.length - 1].x) <
            0.1 &&
          Math.abs(outerPoints[0].z - outerPoints[outerPoints.length - 1].z) <
            0.1);

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
        shapePath: {
          points: pointsWithOffset,
          isClosed: isClosed,
          holes: innerHoles.length > 0 ? innerHoles : undefined,
        },
        height: infraHeight,
        tags,
      });
    }
  });

  result.infras.sort(
    (a, b) => (LAYER_PRIORITY[a.type] || 0) - (LAYER_PRIORITY[b.type] || 0),
  );

  return result;
};
