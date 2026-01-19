// @pxl/bake/terrain/model/osm_feature.ts

import type { f64, i32, u32 } from "@xpute/spec/lib/kit/word.ts";

import type { V3 } from "@pxl/spec/space/v3.ts";
import type { G3857 } from "@pxl/spec/space/adapter/3857.ts";

export const enum OSMType {
  NODE = "node",
  WAY = "way",
  RELATION = "relation",
}

/**
 * [Shared] OSM 태그 정의
 * 태그의 내용은 좌표계와 무관하므로 4326/3857 모든 단계에서 공유합니다.
 * parseOSMData에서 이 태그들을 기준으로 MapInfra와 POI를 분류합니다.
 */
export type OSMTags = {
  /** 공통: 장소나 시설의 고유 이름 */
  name?: string;

  // --- MapInfra 관련 태그 (물리적 형체 위주) ---
  /** 건물 여부 및 종류 */
  building?: string;
  /** 건물 부분체 (복합 건물 구조 표현 시) */
  "building:part"?: string;
  /** 건물의 층수 (높이 계산용) */
  "building:levels"?: string;
  /** 명시적인 건물 최소 층수 */
  "building:min_level"?: string;
  /** 명시적인 건물 높이(m) */
  height?: string;
  /** 지붕 높이(m) */
  "roof:height"?: string;
  /** 도로의 등급 (motorway, primary, residential 등) */
  highway?: string;
  /** 자연 지형 (water, wood, sand 등) */
  natural?: string;
  /** 수계 상세 (river, canal 등) */
  waterway?: string;
  /** 토지 용도 (grass, forest, commercial, residential 등) */
  landuse?: string;
  /** 여가 시설 (park, pitch, playground, golf_course 등) */
  leisure?: string;
  /** 수계 종류 (lake, reservoir, basin 등) */
  water?: string;

  // --- [New] Boundary & Area 관련 태그 ---
  /** 경계선 타입 (administrative 등) */
  boundary?: string;
  /** 행정 구역 레벨 (4: 도/시, 8: 동/면 등) */
  admin_level?: string;
  /** 도로 영역 여부 (면 형태의 도로 렌더링 시) */
  "area:highway"?: string;
  /** 다리 여부 */
  bridge?: string;
  /** 터널 여부 */
  tunnel?: string;
  /** 층수 정보 (미세 고도 스태킹용) */
  layer?: string;
  /** 도로 차선 수 (너비 계산용) */
  lanes?: string;
  /** 도로 폭 (명시적 값) */
  width?: string;
  /** 캠퍼스 여부 */
  campus?: string;

  // --- POI 관련 태그 (정보 및 장소 위주) ---
  /** 생활 편의 시설 (cafe, hospital, school, university, parking 등) */
  amenity?: string;
  /** 상점 종류 (mall, bakery, supermarket 등) */
  shop?: string;
  /** 관광 명소 및 문화 시설 (museum, viewpoint, attraction 등) */
  tourism?: string;
  /** 역사적 가치가 있는 유적지 (monument, castle, ruins 등) */
  historic?: string;
  /** 철도 및 대중교통 거점 (station 등) */
  railway?: string;

  /** 가변적인 모든 태그 허용 */
  [key: string]: string | undefined;
};

/**
 * [Intermediate DTO] Transformer -> Normalizer 전달용 규격
 * Transformer들이 출력하고 Normalizer가 입력받는 표준 중간 데이터 규격입니다.
 * 좌표계는 EPSG:3857(Meter)을 전제로 합니다.
 * Feature와 생명주기를 같이 하는 "내부 데이터"이므로 여기에 있는 것이 적합합니다.
 */
export type OSMRawFeature = {
  id: f64; // 🎯 OSM ID는 u32(42억)를 초과하므로 f64의 Safe Integer 공간 사용
  type: OSMType;
  tags: OSMTags;
  geometry: {
    points: G3857[];
    holes?: G3857[][];
  };
  center?: G3857;
  is_polygon_source?: boolean;
};

/**
 * 엔진 내부 최적화 플래그 집합
 * 태그 문자열 파싱 비용을 줄이기 위해 Normalizer 단계에서 미리 계산된 불리언 값들입니다.
 */
export type OSMExtra = {
  /** 렌더링 우선순위 (Rank)
   * 0~255 범위. 숫자가 높을수록 중요한 지형지물/POI임을 의미합니다.
   * 클라이언트 엔진은 이 값을 보고 겹치는 POI 중 무엇을 먼저 숨길지 결정합니다.
   */
  rank: u32; // 🎯 양수 가중치

  /** 🌊 [New] 해안선 데이터셋에서 추출된 "진짜 바다" 여부 */
  is_ocean: boolean;

  /** 주차장 구역 판별 (amenity=parking, etc.) */
  is_parking: boolean;
  /** 학교, 병원, 관공서 등 대형 시설 구역 판별 */
  is_institutional: boolean;
  /** 스포츠, 운동장 구역 판별 */
  is_sports: boolean;
  // 필요 시 isCampus, isForest 등으로 확장 가능
};

/**
 * 엔진에서 즉시 처리 가능한 상태로 정제된 지형지물입니다.
 * 원본 4326/3857 입력은 정규화 단계에서 canonical PVF3 좌표로 투영되며,
 * 계산된 크기 값들도 엔진 canonical `u` 단위로 변환된 상태입니다.
 */
export type OSMFeature = {
  /** 객체 타입 (node, way, relation) */
  type: OSMType;

  /** OSM 고유 ID (Relation의 경우 음수 혹은 대형 ID가 올 수 있음) */
  id: f64; // 🎯 OSM ID 오버플로우 방지
  /** * 요소의 지리적 중심점
   * 라벨 배치, POI 마커 생성, 카메라 포커싱의 기준점이 됩니다.
   */
  center: V3;

  /** * 해석이 완료된 형상 정보
   * 정규화 단계에서 isClosed 판정과 Holes(inner 멤버) 조립이 끝난 상태입니다.
   * 엔진의 래스터라이저(Rasterizer)는 별도의 기하 판정 없이 이 데이터를 즉시 폴리곤/라인으로 변환합니다.
   */
  shape: {
    points: V3[];
    /** * 폐곡면 여부. tags.landuse나 시작-끝점 일치 여부를 기반으로
     * 면(Fill)으로 채울지 선(Stroke)으로 그릴지 결정합니다.
     */
    is_closed: boolean;
    /** Relation의 'inner' 멤버들이 canonical PVF3로 조립된 구멍(Hole) 데이터 */
    holes?: V3[][];
  };

  /** * [Computed] 엔진이 원본 태그를 바탕으로 계산해낸 물리적 수치
   * tags에서 매번 파싱하지 않도록 전처리기가 계산한 결과값입니다.
   * 렌더링 단계에서 바로 사용할 수 있도록 크기 관련 값은 canonical `u` 단위로 정규화됩니다.
   */
  computed: {
    /** 빌딩 총 높이 (levels * 3.5 + roof_height 등을 합산한 최종 높이) */
    height: f64; // local cell scale
    /** 도로/수계 폭 (lanes 수에 3.5m를 곱하거나 width 태그를 수치화한 값) */
    width: f64; // local cell scale
    /** * 렌더링 우선순위 및 Z-Fighting 방지를 위한 층위 정보.
     * layer 태그 값을 기반으로 하며, 이후 오프셋 연산의 기초가 됩니다.
     */
    layer: i32; // 🎯 터널/지하철을 위한 음수 허용
  };

  /** * [Raw] 원본 소스 태그
   * 정규화 이후에도 메타데이터 참조, 조건부 스타일링, 상세 정보 팝업 출력을 위해 유지합니다.
   */
  tags: OSMTags;

  /**
   * 서비스 계층(GeoService) 또는 정규화기(Normalizer)에서 확정한 렌더링 카테고리입니다.
   * 'building', 'road', 'water', 'forest', 'poi', 'landuse' 등 엔진의 레이어 시스템과 1:1 매칭됩니다.
   * 파서와 엔진은 이 값을 통해 데이터의 용도를 즉시 식별합니다.
   */
  category: string;

  /** [Extra] 엔진이 태그 분석을 통해 추가로 판단한 최적화 플래그 */
  extra: OSMExtra;
};

// ---- OSM 원본 데이터로부터 정제된 개별 객체 모델들을 정의합니다. ----

export type ShapePath = {
  points: V3[];
  is_closed: boolean;
  holes?: V3[][];
};

export type POI = {
  id: string;
  type: "poi";
  category: string;
  name: string;
  center: V3;
  tags: Record<string, string | undefined>;

  /** 엔진이 확정한 중요도 수준 */
  rank: u32;
};
