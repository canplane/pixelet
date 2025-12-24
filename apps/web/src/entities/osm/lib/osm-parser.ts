import type { OverpassElement, OverpassMember } from '../model/types';
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
 * [Advanced Parsing Logic]
 * Overpass API 응답 데이터를 기반으로 3D 환경에 최적화된 인프라 및 POI 데이터를 생성합니다.
 * 선(Way) 데이터와 점(Node) 데이터를 엄격히 분리하여 도로가 점으로 보이는 현상을 방지합니다.
 */
export const parseOsmData = (
  elements: OverpassElement[],
  project: (loc: Location) => MeterPos | null,
): ParseResult => {
  const result: ParseResult = { infras: [], pois: [] };
  if (!elements) return result;

  elements.forEach((item: OverpassElement, idx: number): void => {
    const tags: Record<string, string | undefined> = item.tags || {};
    const rawLat: number | undefined = item.lat || item.center?.lat;
    const rawLon: number | undefined = item.lon || item.center?.lon;

    // 1. POI 정밀 파싱 (Node 타입 처리)
    // 도로 구성 요소 중 Node에 highway 태그가 붙은 경우(교차점 등)는 인프라가 아닌 POI/데이터로 처리하여
    // 베이커에서 선(Way)이 점(Node)에 의해 덮어씌워지는 것을 방지합니다.
    if (item.type === 'node') {
      if (tags.name && rawLat && rawLon) {
        const poiPos: MeterPos | null = project({
          lng: rawLon,
          lat: rawLat,
          alt: 0,
        });
        if (poiPos) {
          result.pois.push({
            id: `poi-${item.id || idx}`,
            type: 'poi',
            category:
              tags.amenity ||
              tags.shop ||
              tags.tourism ||
              tags.highway || // 교차로 등
              'place',
            name: tags.name,
            center: { ...poiPos },
            tags,
          });
        }
      }
      return; // Node 타입은 여기서 종료
    }

    // 2. 인프라 지오메트리 처리 (Way & Relation 전용)
    if (item.type === 'way' || item.type === 'relation') {
      const outerPoints: MeterPos[] = [];
      const innerHoles: MeterPos[][] = [];

      // out body geom 쿼리 대응: item.geometry에 직접 좌표 배열이 포함됨
      if (item.geometry) {
        item.geometry.forEach((pt: { lat: number; lon: number }): void => {
          const projected: MeterPos | null = project({
            lng: pt.lon,
            lat: pt.lat,
            alt: 0,
          });
          if (projected) outerPoints.push(projected);
        });
      } else if (item.type === 'relation' && item.members) {
        item.members.forEach((member: OverpassMember): void => {
          if (!member.geometry) return;
          const memberPoints: MeterPos[] = member.geometry
            .map((pt: { lat: number; lon: number }): MeterPos | null =>
              project({ lng: pt.lon, lat: pt.lat, alt: 0 }),
            )
            .filter((v: MeterPos | null): v is MeterPos => v !== null);

          if (member.role === 'outer') {
            outerPoints.push(...memberPoints);
          } else if (member.role === 'inner') {
            innerHoles.push(memberPoints);
          }
        });
      }

      // 유효한 지오메트리 검사 (최소 2개 이상의 점 필요)
      if (outerPoints.length < 2) return;

      // [고도화] 타입 판정 엔진
      const isBuilding: boolean = !!(tags.building || tags['building:part']);
      const isRoad: boolean = !!(
        tags.highway ||
        tags.railway ||
        tags['area:highway']
      );
      const isWater: boolean =
        tags.natural === 'water' ||
        !!tags.waterway ||
        tags.landuse === 'reservoir' ||
        tags.water === 'river';
      const isLanduse: boolean = !!(
        tags.landuse ||
        tags.leisure ||
        tags.natural
      );

      if (!isBuilding && !isRoad && !isWater && !isLanduse) return;

      // [면적 대응] 도로 데이터의 너비 추론
      // 베이커에서 thickness로 변환될 핵심 데이터
      if (isRoad) {
        const lanes: number = tags.lanes ? parseInt(tags.lanes) : 1;
        const widthTag: number = tags.width ? parseFloat(tags.width) : 0;
        // 차선당 약 3.5m 기본값 적용 및 태그 우선순위 부여
        const calculatedWidth: number = widthTag > 0 ? widthTag : lanes * 3.5;
        tags._calculatedWidth = calculatedWidth.toString();
      }

      // 바운딩 박스 계산 (Floating Origin 대응)
      let minX: number = Infinity;
      let maxX: number = -Infinity;
      let minZ: number = Infinity;
      let maxZ: number = -Infinity;

      outerPoints.forEach((p: MeterPos): void => {
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

      // [고도화] 수직 오프셋(Y-Axis) 및 레이어 스태킹
      const layerLevel: number = tags.layer ? parseFloat(tags.layer) : 0;
      const pointsWithOffset: MeterPos[] = outerPoints.map(
        (pt: MeterPos): MeterPos => {
          let y: number = 0;
          if (isRoad) {
            y = 0.05 + layerLevel * 0.2;
            if (tags.bridge === 'yes') y += 1.0;
            if (tags.tunnel === 'yes') y -= 1.0;
          } else if (isWater) {
            y = -0.15 + layerLevel * 0.1;
          } else if (isLanduse) {
            y = 0.02 + (tags.landuse === 'forest' ? 0.03 : 0);
          }
          return { ...pt, y };
        },
      );

      // 건물 높이 계산
      let infraHeight: number = 0;
      if (isBuilding) {
        const levels: number = tags['building:levels']
          ? parseFloat(tags['building:levels'])
          : 3;
        const minLevel: number = tags['building:min_level']
          ? parseFloat(tags['building:min_level'])
          : 0;
        const roofHeight: number = tags['roof:height']
          ? parseFloat(tags['roof:height'])
          : 0;
        const baseHeight: number = tags.height
          ? parseFloat(tags.height)
          : (levels - minLevel) * 3.5 + roofHeight;
        infraHeight = Math.max(0, baseHeight);
      }

      // 지오메트리 위상 분석 (도형 닫힘 여부)
      // 도로가 아닌 경우 3개 이상의 점이 있고 시작/끝이 같으면 닫힌 도형으로 간주
      const isClosed: boolean =
        item.type === 'relation' ||
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

      let infraType: 'building' | 'road' | 'water' | 'landuse' = 'landuse';
      if (isBuilding) infraType = 'building';
      else if (isRoad) infraType = 'road';
      else if (isWater) infraType = 'water';

      result.infras.push({
        id: `infra-${item.id || idx}`,
        type: infraType,
        points: pointsWithOffset,
        center: center,
        shapePath: shapePath,
        height: infraHeight,
        tags: tags,
      });
    }
  });

  return result;
};
