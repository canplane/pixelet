// // @pxl/web/modules/world/poi/poi-manager.ts

// import React, { useRef, useState } from "react";
// import { useFrame } from "@react-three/fiber";
// import { useMapStore } from "@/modules/map/store.ts";
// import { POIItem } from "./poi-item.tsx";
// import { POIInstancedMesh } from "./poi-instanced-mesh.tsx";
// import type { POI } from "@pxl/bake/terrain/model/osm_feature.ts";

// // 🎯 [Constraint] HTML로 렌더링할 최대 개수 (성능 마지노선)
// const MAX_HTML_POIS = 20;

// // 🎯 [Throttle] 정렬 로직 실행 주기 (프레임 드랍 방지, 0.2초)
// const SORT_INTERVAL = 0.2;

// type POIManagerProps = {
//   pois: POI[]; // WorldLayer에서 넘겨준 10,000개 데이터
// };
// export const POIManager = React.memo(({ pois }: POIManagerProps) => {
//   const [nearbyPOIs, setNearbyPOIs] = useState<POI[]>([]);

//   const lastSortTime = useRef<number>(0);

//   /**
//    * [Hybrid-LOD-Selector]
//    * 매 프레임 카메라와의 거리를 계산하여 'HTML로 승격될' 상위 N개를 선별합니다.
//    */
//   useFrame((state) => {
//     const now = state.clock.getElapsedTime();

//     // 1. Throttle: 매 프레임 정렬은 10,000개 기준 너무 무겁습니다.
//     if (now - lastSortTime.current < SORT_INTERVAL) return;
//     lastSortTime.current = now;

//     const cameraPos = state.camera.position;
//     const floatingOrigin = useMapStore.getState().floatingOrigin; // 🎯 최신 오리진 조회

//     // 2. 거리 기반 정렬 (Nearest Neighbor Search)
//     // 10,000개 정렬은 최신 JS 엔진에서 약 2~4ms 소요 (허용 가능)
//     // Web Worker로 빼면 더 좋지만, 데이터 전송 오버헤드 고려 시 메인 스레드도 괜찮음.
//     const sortedIndices = pois
//       .map((poi, index) => {
//         // 로컬 좌표로 변환하여 거리 계산
//         const lx = poi.center.x - floatingOrigin.x;
//         const lz = -(poi.center.y - floatingOrigin.y); // Y -> -Z
//         // 3D 거리(높이 포함)보다는 평면 거리만으로도 충분
//         const distSq = (lx - cameraPos.x) ** 2 + (lz - cameraPos.z) ** 2;
//         return { index, distSq };
//       })
//       .sort((a, b) => a.distSq - b.distSq);

//     // 3. 상위 N개 추출
//     const topN = sortedIndices
//       .slice(0, MAX_HTML_POIS)
//       .map((item) => pois[item.index]);

//     // 4. 상태 업데이트 (Deep Comparison을 통해 불필요한 리렌더링 방지)
//     // 첫 번째 아이템의 ID가 바뀌었거나, 길이가 다를 때만 업데이트
//     const isChanged = topN.length !== nearbyPOIs.length ||
//       (topN.length > 0 && topN[0].id !== nearbyPOIs[0].id);

//     if (isChanged) {
//       setNearbyPOIs(topN);
//     }
//   });

//   return (
//     <>
//       {/* 1. 배경 Layer: 10,000개 전체를 점(Mesh)으로 그림 */}
//       {/* POIInstancedMesh 내부에서 '가까운 POI'는 Scale=0으로 숨기는 로직이 있어야 완벽함 */}
//       <POIInstancedMesh pois={pois} />

//       {/* 2. 전경 Layer: 선별된 20개만 고비용 HTML 렌더링 */}
//       {nearbyPOIs.map((poi) => <POIItem key={poi.id} poi={poi} />)}
//     </>
//   );
// });
