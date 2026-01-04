import type { Coord } from '@pixelet/core/shared/model/coords';
import type { OsmElementType, OsmTags } from './osm-source';

/**
 * [Intermediate DTO] Transformer -> Normalizer 전달용 규격
 * Transformer들이 출력하고 Normalizer가 입력받는 표준 중간 데이터 규격입니다.
 * 좌표계는 EPSG:3857(Meter)을 전제로 합니다.
 * Feature와 생명주기를 같이 하는 "내부 데이터"이므로 여기에 있는 것이 적합합니다.
 */
export interface OsmInput {
  id: number;
  type: OsmElementType;
  tags: OsmTags;
  geometry: {
    points: Coord[];
    holes?: Coord[][];
  };
  center?: Coord;
  isPolygonSource?: boolean;
}

/**
 * 🚀 [New] 엔진 내부 최적화 플래그 집합
 * 태그 문자열 파싱 비용을 줄이기 위해 Normalizer 단계에서 미리 계산된 불리언 값들입니다.
 */
export interface OsmExtra {
  /** * 🚀 [New] 렌더링 우선순위 (Rank)
   * 0~255 범위. 숫자가 높을수록 중요한 지형지물/POI임을 의미합니다.
   * 클라이언트 엔진은 이 값을 보고 겹치는 POI 중 무엇을 먼저 숨길지 결정합니다.
   */
  rank: number;

  /** 🌊 [New] 해안선 데이터셋에서 추출된 "진짜 바다" 여부 */
  isOcean: boolean;

  /** 주차장 구역 판별 (amenity=parking, etc.) */
  isParking: boolean;
  /** 학교, 병원, 관공서 등 대형 시설 구역 판별 */
  isInstitutional: boolean;
  /** 스포츠, 운동장 구역 판별 */
  isSports: boolean;

  // 필요 시 isCampus, isForest 등으로 확장 가능
}

/**
 * [osm-feature]
 * 엔진에서 렌더링 가능한 상태로 정제된 지형지물(EPSG:3857, Web Mercator)입니다.
 * 4326 좌표계가 3857 미터 좌표계로 투영(Projection)되었으며,
 * 모든 물리적 속성(높이, 너비)이 수치(number)로 변환되었습니다.
 */
export interface OsmFeature {
  /** 객체 타입 (node, way, relation) */
  type: OsmElementType;

  /** OSM 고유 ID (Relation의 경우 음수 혹은 대형 ID가 올 수 있음) */
  id: number;

  /** * 요소의 지리적 중심점 (EPSG:3857 Meter)
   * 라벨 배치, POI 마커 생성, 카메라 포커싱의 기준점이 됩니다.
   */
  center: Coord;

  /** * 해석이 완료된 형상 정보
   * 정규화 단계에서 isClosed 판정과 Holes(inner 멤버) 조립이 끝난 상태입니다.
   * 엔진의 래스터라이저(Rasterizer)는 별도의 기하 판정 없이 이 데이터를 즉시 폴리곤/라인으로 변환합니다.
   */
  shape: {
    /** 3857 미터 단위로 투영된 좌표 리스트 */
    points: Coord[];
    /** * 폐곡면 여부. tags.landuse나 시작-끝점 일치 여부를 기반으로
     * 면(Fill)으로 채울지 선(Stroke)으로 그릴지 결정합니다.
     */
    isClosed: boolean;
    /** Relation의 'inner' 멤버들이 미터 좌표로 조립된 구멍(Hole) 데이터 */
    holes?: Coord[][];
  };

  /** * [Computed] 엔진이 원본 태그를 바탕으로 계산해낸 물리적 수치
   * tags에서 매번 파싱하지 않도록 전처리기가 계산한 결과값입니다.
   * 렌더링 성능 최적화를 위해 모든 단위는 SI 표준(Meter)을 따릅니다.
   */
  computed: {
    /** 빌딩 총 높이 (levels * 3.5 + roof_height 등을 합산한 최종 높이) */
    height: number;
    /** 도로/수계 폭 (lanes 수에 3.5m를 곱하거나 width 태그를 수치화한 값) */
    width: number;
    /** * 렌더링 우선순위 및 Z-Fighting 방지를 위한 층위 정보.
     * layer 태그 값을 기반으로 하며, 이후 오프셋 연산의 기초가 됩니다.
     */
    layer: number;
  };

  /** * [Raw] 원본 소스 태그
   * 정규화 이후에도 메타데이터 참조, 조건부 스타일링, 상세 정보 팝업 출력을 위해 유지합니다.
   */
  tags: OsmTags;

  /**
   * 서비스 계층(GeoService) 또는 정규화기(Normalizer)에서 확정한 렌더링 카테고리입니다.
   * 'building', 'road', 'water', 'forest', 'poi', 'landuse' 등 엔진의 레이어 시스템과 1:1 매칭됩니다.
   * 파서와 엔진은 이 값을 통해 데이터의 용도를 즉시 식별합니다.
   */
  category: string;

  /** [Extra] 엔진이 태그 분석을 통해 추가로 판단한 최적화 플래그 */
  extra: OsmExtra;
}

/**
 * OSM 원본 데이터로부터 정제된 개별 객체 모델들을 정의합니다.
 */

export interface ShapePath {
  points: Coord[];
  isClosed: boolean;
  holes?: Coord[][];
}

export interface MapPoi {
  id: string;
  type: 'poi';
  category: string;
  name: string;
  center: Coord;
  tags: Record<string, string | undefined>;

  /** 엔진이 확정한 중요도 수준 */
  rank: number;
}
