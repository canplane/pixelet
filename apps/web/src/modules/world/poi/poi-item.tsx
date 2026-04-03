// // @pxl/web/modules/world/poi/poi-item.ts

// import { memo, type ReactElement, useCallback, useMemo, useRef, useState } from "react";
// import * as THREE from "three";
// import { useFrame } from "@react-three/fiber";
// import { Html } from "@react-three/drei";
// import { useMapStore } from "@/modules/map/store.ts";
// import { type TileState, useTileStore } from "@/domain/geo/model/tile-store.ts";
// import { useMapNavigation } from "../../model/use-map-navigation.ts";
// import type { POI } from "@pxl/bake/terrain/model/osm_feature.ts";
// import { render_cfg } from  "@/domain/three/config.ts";
// import { cn } from "@/shared/lib/utils/cn.ts";
// import { getTileCoord, getTileKey, toCenter } from "@pxl/bake/terrain/etc/tile-utils.ts";
// import { useGroundHeight } from "../../lib/use-ground-height.ts";
// import { to3D, toThreeCoord } from "@/modules/world/render/config.ts";

// // 🚀 [Refactor] 좌표계 훅 & 스타일 설정 분리
// import { useRelativePosition } from "../../lib/use-coordinate-converter.ts";
// import { resolvePoiStyle } from "./poi-theme-config.ts";

// // 🎯 [Optimization] 렌더링 성능을 위한 상수 캐싱
// const { FADE_START, FADE_END, FAR_CLIP } = render_cfg;
// // HTML 요소는 WebGL보다 무거우므로, Far Clip보다 조금 더 일찍 숨겨서 DOM 부하를 줄입니다.
// const CULL_DIST = FAR_CLIP * 0.7;

// /**
//  * 사용자와 상호작용 가능한 근거리 POI 라벨을 렌더링합니다.
//  * * Architecture:
//  * - Global State (Origin) -> useRelativePosition Hook -> Local Coord
//  * - Config File -> resolvePoiStyle -> Visual Style
//  */

// type POIItemProps = {
//   poi: POI;
// };
// export const POIItem = memo(({ poi }: POIItemProps) => {
//   const { get_ele } = useGroundHeight();

//   // ---- [Global Store Selectors] ----
//   const html_portal_el = useMapStore((state) => state.html_portal_el);
//   const set_selected_poi = useMapStore((state) => state.set_selected_poi);

//   // ---- [Theme Context] ----
//   const { themeKey } = useMapNavigation();
//   const isNight: boolean = themeKey === "night";

//   // ---- [Refs & Local State] ----
//   const groupRef = useRef<THREE.Group>(null);
//   const containerRef = useRef<HTMLDivElement>(null);

//   const [isHovered, setIsHovered] = useState<boolean>(false);
//   const [isOccluded, setIsOccluded] = useState<boolean>(false);

//   // ---- [Animation Refs] (Re-render 방지용 Mutable Variables) ----
//   const springValue = useRef<number>(0);
//   const velocity = useRef<number>(0);
//   const lastIsMinimal = useRef<boolean>(false);

//   // 렌더링 트리 변경을 유발하는 상태들
//   const [isMinimal, setIsMinimal] = useState<boolean>(false);
//   const [isCulled, setIsCulled] = useState<boolean>(false);

//   const shouldShowName: boolean = !isMinimal || isHovered;

//   // =========================================================================
//   // SECTION 1. [Data Preparation] 높이 계산 및 스타일 로딩
//   // =========================================================================

//   // 1-1. [Terrain-Snap] 페이지 로딩 상태 정밀 구독
//   const tileKey = useMemo(() => getTileKey(getTileCoord(poi.center)), [poi.center]);
//   // useCallback으로 Selector 참조를 유지하여 불필요한 구독 갱신 방지
//   const isTileLoadedSelector = useCallback((state: TileState) => state.getPage(tileKey), [tileKey]);
//   const isTileLoaded = useTileStore(isTileLoadedSelector);

//   // 1-2. [Height Calculation] 지형 높이 샘플링 (O(1))
//   // 페이지이 로드되면 높이를 재계산하여 바닥에 붙는 현상 방지
//   const terrainHeight = useMemo(() => {
//     return get_ele(toCenter(poi.center));
//   }, [poi.center.x, poi.center.y, isTileLoaded, get_ele]);

//   // 1-3. 🚀 [Refactor: Coordinate Hook]
//   // 절대 좌표(Map) -> 상대 좌표(Scene) 변환을 훅으로 위임 (Origin 변경 시 자동 반응)
//   const relativePos = useRelativePosition(poi.center);

//   // 1-4. 🚀 [Refactor: Style Config]
//   // 복잡한 스타일 맵을 외부 설정 파일로 분리하여 가독성 확보
//   const categoryInfo = useMemo(() => resolvePoiStyle(poi.category, isNight), [poi.category, isNight]);

//   // =========================================================================
//   // SECTION 2. [Frame Loop] 위치 동기화 및 애니메이션 (60FPS)
//   // =========================================================================
//   useFrame((state, delta): void => {
//     if (!groupRef.current || !containerRef.current) return;

//     // 2-1. [Position Sync] Pipeline Integration
//     // Hook에서 계산된 '상대 좌표(relativePos)'를 기반으로 최종 씬 좌표를 계산합니다.
//     // (1) Center Align (복셀 정렬) -> (2) 3D Projection (높이 적용) -> (3) Axis Swap
//     const alignedPos = toCenter(relativePos);
//     const [tPosX, tPosY, tPosZ] = toThreeCoord(
//       to3D(alignedPos, terrainHeight + 0.5), // UI 오프셋 0.5m 추가
//     );

//     groupRef.current.position.set(tPosX, tPosY, tPosZ);

//     // 2-2. [Culling] 거리 기반 렌더링 최적화
//     const dist = state.camera.position.distanceTo(groupRef.current.position);

//     if (dist > CULL_DIST) {
//       if (!isCulled) setIsCulled(true);
//       return; // 멀리 있으면 이후 연산 스킵
//     } else {
//       if (isCulled) setIsCulled(false);
//     }

//     // 2-3. [Minimal Mode] 거리에 따른 UI 축소 전환
//     const minimalThreshold: number = FADE_START * 0.3;
//     const currentMinimal: boolean = dist > minimalThreshold;
//     if (currentMinimal !== lastIsMinimal.current) {
//       lastIsMinimal.current = currentMinimal;
//       setIsMinimal(currentMinimal);
//     }

//     // 2-4. [Opacity Calculation] 거리 기반 페이드 아웃
//     let targetOpacity: number = 1;
//     if (dist > FADE_START) {
//       targetOpacity = THREE.MathUtils.mapLinear(dist, FADE_START, FADE_END, 1, 0);
//     }

//     // 건물에 가려졌거나(Occluded) 너무 멀면 투명하게 처리
//     const finalOpacity: number = isOccluded || dist > FADE_END ? 0 : Math.max(0, targetOpacity);
//     const isVisible: boolean = finalOpacity > 0.01;

//     // 2-5. [Physics Animation] Spring Logic
//     // React State 대신 Ref를 사용하여 JS Thread 부하 없이 애니메이션 수행
//     const targetScale: number = isVisible ? 1 : 0;
//     // F = -kx - cv (Hooke's Law + Damping)
//     const ax: number = -250 * (springValue.current - targetScale) - 15 * velocity.current;

//     velocity.current += ax * delta;
//     springValue.current += velocity.current * delta;

//     // DOM 직접 조작 (Ref)
//     const element: HTMLDivElement = containerRef.current;
//     if (!isVisible && springValue.current < 0.001) {
//       element.style.display = "none";
//     } else {
//       element.style.display = "flex";
//       element.style.opacity = finalOpacity.toString();
//       element.style.transform = `scale(${springValue.current})`;
//     }
//   });

//   // 🎯 컬링된 상태면 React Tree에서 제외하여 성능 확보
//   if (isCulled) return null;

//   return (
//     <group ref={groupRef}>
//       <Html
//         portal={html_portal_el ? { current: html_portal_el } : undefined}
//         position={fill3(0)} // Group이 이미 위치를 잡았으므로 로컬 0,0,0
//         center
//         occlude
//         onOcclude={setIsOccluded}
//         sprite
//         style={{ pointerEvents: "none" }} // HTML 영역 외 클릭 투과
//         zIndexRange={[100, 0]} // 거리에 따른 정렬 순서 보장
//       >
//         <div ref={containerRef} className="flex flex-col items-center origin-bottom select-none transition-transform duration-300">
//           {/* --- [Badge Container] --- */}
//           <div
//             onClick={(e: React.MouseEvent): void => {
//               e.stopPropagation();
//               set_selected_poi(poi);
//             }}
//             onPointerOver={(): void => setIsHovered(true)}
//             onPointerOut={(): void => setIsHovered(false)}
//             className={cn(
//               "flex items-center rounded-full border transition-all duration-500 ease-out cursor-pointer pointer-events-auto",
//               "antialiased tracking-tighter",
//               isNight
//                 ? "bg-slate-900/70 backdrop-blur-md border-white/10 shadow-[0_8px_16px_rgba(0,0,0,0.4)]"
//                 : "bg-white/85 backdrop-blur-[6px] border-white/60 shadow-[0_4px_12px_rgba(0,0,0,0.08)]",
//               isHovered ? "-translate-y-2" : "translate-y-0",
//               shouldShowName ? "gap-1.5 px-2 py-1" : "gap-0 p-1",
//             )}
//           >
//             {/* Icon Circle */}
//             <div
//               className={cn(
//                 "w-5 h-5 flex-none flex items-center justify-center rounded-full text-[10px] transition-all duration-500",
//                 categoryInfo.color,
//                 isHovered && categoryInfo.glow,
//                 isMinimal && !isHovered && "scale-75",
//               )}
//             >
//               <span className="leading-none">{categoryInfo.icon}</span>
//             </div>

//             {/* Text Label */}
//             <div className={cn("overflow-hidden transition-all duration-500 ease-in-out", shouldShowName ? "max-w-[200px] opacity-100" : "max-w-0 opacity-0")}>
//               <span
//                 className={cn(
//                   "text-[12px] font-bold pr-1 select-none leading-none pt-px block ml-1 whitespace-nowrap",
//                   isNight ? "text-white/90" : "text-slate-800",
//                 )}
//               >
//                 {poi.name}
//               </span>
//             </div>
//           </div>

//           {/* --- [Anchor Dot] --- */}
//           <div
//             className={cn(
//               "w-1.5 h-1.5 mx-auto rounded-full mt-1.5 border-2 transition-all duration-500",
//               isNight ? "bg-white/60 border-white/20" : "bg-slate-500 border-white",
//               isMinimal && !isHovered ? "scale-0" : "scale-100",
//               isHovered && "scale-150 bg-blue-400 border-blue-100",
//             )}
//           />
//         </div>
//       </Html>
//     </group>
//   );
// });
// PoiItem.displayName = "PoiItem";
