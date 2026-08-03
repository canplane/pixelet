// @pixelet/server/bake/terrain/pipeline/poi_extractor.ts

import type { OSMFeature, POI } from "@pixelet/shared/bake/model/osm_feature.ts";

/** 지도에 표시할 중요한 장소(POI)들을 선별합니다. */
export function extract_pois(feats: OSMFeature[]): POI[] {
  const pois: POI[] = [];

  for (const feat of feats) {
    const { tags, type, category, center, extra, id } = feat;
    const { name } = tags;

    // 🌊 [Guard] 바다는 절대 POI가 될 수 없음 + 필수 데이터 검증
    if (extra.is_ocean || !center || !name) continue;

    // 🎯 [Refine] 수집 정책 보강
    const is_explicit_poi = category === "poi";
    const is_major_facility = extra.is_institutional || extra.is_sports;

    // [Named Node] 단순히 모든 노드를 가져오지 않고,
    // 최소한의 장소 정보(amenity, shop, tourism 등)가 있는 노드만 선별
    const is_named_node = type === "node" && (!!tags.amenity || !!tags.shop || !!tags.tourism || !!tags.place || !!tags.historic);

    // [Policy Check]
    if (is_explicit_poi || is_major_facility || is_named_node) {
      pois.push({
        id: `poi-${id}`,
        type: "poi",
        // 우선순위: amenity > shop > tourism > category
        category: tags.amenity || tags.shop || tags.tourism || category || "place",
        name,
        center,
        rank: extra.rank, // Normalizer에서 이미 주입됨
        tags,
      });
    }
  }

  // 📈 [Density Control] 랭크 순 정렬 (클라이언트 렌더링 최적화 힌트)
  return pois.sort((a, b) => b.rank - a.rank);
}
