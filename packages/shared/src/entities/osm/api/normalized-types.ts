/**
 * @file normalized-types.ts
 * 엔진 내부에서 정규화(EPSG:3857 Meter) 및 전처리가 완료된 데이터 규격입니다.
 * 원본 Overpass/PostGIS raw 데이터를 수용하되, 렌더링 엔진과 베이커(Baker)가
 * 즉시 소비할 수 있도록 물리적 수치와 카테고리 정책이 확정된 상태입니다.
 */

import type { MeterPos } from '@pixelet/shared/shared/model/coords';
import type { ElementType, OsmTags } from './overpass-types';

/**
 * [엔진 표준] 정규화된 개별 요소 (NormalizedElement)
 * 모든 좌표는 WGS84(4326)에서 웹 메르카토르(3857) 미터 단위로 변환되어 있습니다.
 */
export interface NormalizedElement {
  /** 객체 타입 (node, way, relation) */
  type: ElementType;

  /** OSM 고유 ID (Relation의 경우 음수 혹은 대형 ID가 올 수 있음) */
  id: number;

  /** * 요소의 지리적 중심점 (EPSG:3857 Meter)
   * 라벨 배치, POI 마커 생성, 카메라 포커싱의 기준점이 됩니다.
   */
  center: MeterPos;

  /** * 해석이 완료된 형상 정보
   * 정규화 단계에서 isClosed 판정과 Holes(inner 멤버) 조립이 끝난 상태입니다.
   * 엔진의 래스터라이저(Rasterizer)는 별도의 기하 판정 없이 이 데이터를 즉시 폴리곤/라인으로 변환합니다.
   */
  shape: {
    /** 3857 미터 단위로 투영된 좌표 리스트 */
    points: MeterPos[];
    /** * 폐곡면 여부. tags.landuse나 시작-끝점 일치 여부를 기반으로
     * 면(Fill)으로 채울지 선(Stroke)으로 그릴지 결정합니다.
     */
    isClosed: boolean;
    /** Relation의 'inner' 멤버들이 미터 좌표로 조립된 구멍(Hole) 데이터 */
    holes?: MeterPos[][];
  };

  /** * 수치화된 물리 데이터 (Computed Properties)
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

  /** * 원본 태그
   * 정규화 이후에도 메타데이터 참조, 조건부 스타일링, 상세 정보 팝업 출력을 위해 유지합니다.
   */
  tags: OsmTags;

  /**
   * 서비스 계층(GeoService) 또는 정규화기(Normalizer)에서 확정한 렌더링 카테고리입니다.
   * 'building', 'road', 'water', 'forest', 'poi', 'landuse' 등 엔진의 레이어 시스템과 1:1 매칭됩니다.
   * 파서와 엔진은 이 값을 통해 데이터의 용도를 즉시 식별합니다.
   */
  category: string;
}
