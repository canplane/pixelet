import React, {
  useMemo,
  useRef,
  useState,
  useCallback,
  type ReactElement,
} from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import { Html } from '@react-three/drei';
import { useMapStore } from '../../model/map-store';
import { useMapNavigation } from '../../model/use-map-navigation';
import type { MapPoi } from '@pixelet/shared/entities/osm/model/osm-feature';
import { THREE_CONFIG } from '../../config/three-config';
import { cn } from '@/shared/lib/utils';
import { VOXEL_CONFIG } from '@pixelet/shared/entities/voxel/voxel-config';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import {
  getTileBbox,
  getTileCoord,
  getTileKey,
} from '@pixelet/shared/entities/osm/lib/utils/tile-utils';
import type { Coord } from '@pixelet/shared/shared/model/coords.d';

const { FADE_START, FADE_END, FAR_CLIP } = THREE_CONFIG.RENDERING;
const { TILE_SIZE, CELL_SIZE, Z_SCALE_RATIO, TILE_RESOLUTION } = OSM_CONSTANTS;

// 🎯 [Optimization] 너무 먼 거리의 DOM은 아예 렌더링하지 않음 (성능 보호)
// HTML 요소는 WebGL보다 비용이 비싸므로 렌더링 한계보다 짧게 잡습니다.
const CULL_DISTANCE = FAR_CLIP * 0.7;

interface PoiItemProps {
  poi: MapPoi;
  // 🎯 [Refactoring] origin Prop 제거.
  // 위치는 useFrame에서 Store를 직접 구독하여 프레임 단위로 동기화하므로 Props 업데이트 불필요.
}

/**
 * [Helper] 특정 절대 좌표(Coord)의 지형 높이를 계산합니다.
 * 셰이더 변위(Vertex Displacement)와 동일한 로직으로 CPU 상에서 높이를 샘플링합니다.
 */
function getTerrainAltitude(pos: Coord): number {
  const tiles = useMapStore.getState().tiles; // 구독 없이 일회성 조회 (성능 최적화)

  // 1. 타일 키 계산 (ex: "12:5")
  const coord = getTileCoord(pos);
  const key = getTileKey(coord);
  const bbox = getTileBbox(coord);

  const tile = tiles.get(key);

  if (!tile) return 0; // 타일이 아직 로드되지 않았으면 해수면(0) 반환

  // 2. 타일 내 로컬 그리드 인덱스 계산
  const localX = pos.x - bbox.minX;
  const localY = pos.y - bbox.minY; // Bottom-Up (OSM Standard)

  // 그리드 좌표 (0 ~ 2047)
  const gx = Math.floor(localX / CELL_SIZE);
  // 🎯 [Correction] GridTerrain 데이터는 Top-Down 이미지 순서로 저장됨을 가정
  const gy = Math.floor((TILE_SIZE - localY) / CELL_SIZE);

  // 범위 체크
  if (gx < 0 || gx >= TILE_RESOLUTION || gy < 0 || gy >= TILE_RESOLUTION)
    return 0;

  // 3. 높이 샘플링 (Uint16 Array Access)
  const index = gy * TILE_RESOLUTION + gx;
  const heightValue = tile.grids.terrain.height[index];

  // 4. 실제 미터 단위 높이 변환
  return (heightValue || 0) * CELL_SIZE * Z_SCALE_RATIO;
}

/**
 * [PoiItem]
 * Floating Origin이 적용된 부모 그룹 내에서 정적인 로컬 좌표를 가집니다.
 * 더 이상 매 프레임 카메라 위치를 역산하지 않으며, 부모 그룹의 이동에 따라 자동으로 포탈 위치가 동기화됩니다.
 */
export const PoiItem = React.memo(
  ({ poi }: PoiItemProps): ReactElement | null => {
    const htmlPortalElement = useMapStore((state) => state.htmlPortalElement);
    const setSelectedPoi = useMapStore((state) => state.setSelectedPoi);

    const { themeKey } = useMapNavigation();
    const isNight: boolean = themeKey === 'night';

    // 🎯 [Scene-Graph] 위치 제어를 위한 Group Ref (HTML 감싸기용)
    const groupRef = useRef<THREE.Group>(null);
    const containerRef = useRef<HTMLDivElement>(null);

    const [isHovered, setIsHovered] = useState<boolean>(false);
    const [isOccluded, setIsOccluded] = useState<boolean>(false);

    // 상태 최적화를 위한 Refs
    const springValue = useRef<number>(0);
    const velocity = useRef<number>(0);
    const lastIsMinimal = useRef<boolean>(false);
    const [isMinimal, setIsMinimal] = useState<boolean>(false);
    const [isCulled, setIsCulled] = useState<boolean>(false); // 🎯 거리 기반 렌더링 차단

    const shouldShowName: boolean = !isMinimal || isHovered;

    // 🎯 [Terrain-Snap-Correction]
    // 1. 이 POI가 속한 타일의 Key를 계산합니다.
    const tileKey = useMemo(
      () => getTileKey(getTileCoord(poi.center)),
      [poi.center],
    );

    /**
     * 2. [State-Subscription] 해당 타일의 로딩 여부만 정밀하게 구독합니다.
     * useCallback을 사용하여 Selector 함수의 참조를 유지해야 불필요한 리렌더링을 막습니다.
     * tiles Map 전체가 바뀌어도, 내 키(tileKey)의 존재 여부(boolean)가 안 바뀌면 리렌더링 되지 않습니다.
     */
    const isTileLoadedSelector = useCallback(
      (state: any) => state.tiles.has(tileKey),
      [tileKey],
    );
    const isTileLoaded = useMapStore(isTileLoadedSelector);

    /**
     * [Terrain-Snap] & [Lazy-Correction]
     * POI의 절대 좌표는 불변이므로, 해당 위치의 지형 높이를 초기 1회 계산합니다.
     * 🎯 [Fix] isTileLoaded가 false -> true로 변하는 순간(타일 로드 완료 시) 재계산하여,
     * 초기 로딩 시 POI가 바닥(0m)에 붙어있는 문제를 해결합니다.
     */
    const terrainHeight = useMemo(() => {
      // 🎯 POI 중심점에 VOXEL_CONFIG.HALF(복셀 중심 보정)를 더해 정확한 샘플링
      return getTerrainAltitude({
        x: poi.center.x + VOXEL_CONFIG.HALF,
        y: poi.center.y + VOXEL_CONFIG.HALF,
        z: 0,
      });
    }, [poi.center.x, poi.center.y, isTileLoaded]);

    /**
     * [get-category-style]
     * POI 카테고리 및 테마(낮/밤)에 따른 시각적 에셋과 컬러셋을 결정합니다.
     */
    const categoryInfo = useMemo((): {
      icon: string;
      color: string;
      glow: string;
    } => {
      const c: string = poi.category;
      const themeStyles: Record<
        string,
        { icon: string; color: string; glow: string }
      > = {
        cafe: {
          icon: '☕',
          color: isNight
            ? 'bg-[#A35D36] text-[#FFD1B3]'
            : 'bg-[#FFD1B3] text-[#A35D36]',
          glow: 'shadow-[0_0_15px_rgba(163,93,54,0.4)]',
        },
        restaurant: {
          icon: '🍴',
          color: isNight
            ? 'bg-[#B04A43] text-[#FFB7B2]'
            : 'bg-[#FFB7B2] text-[#B04A43]',
          glow: 'shadow-[0_0_15px_rgba(176,74,67,0.4)]',
        },
        shop: {
          icon: '🛍️',
          color: isNight
            ? 'bg-[#4A6DAB] text-[#B2CEFE]'
            : 'bg-[#B2CEFE] text-[#4A6DAB]',
          glow: 'shadow-[0_0_15px_rgba(74,109,171,0.4)]',
        },
        tourism: {
          icon: '🏛️',
          color: isNight
            ? 'bg-[#7A5DAC] text-[#E2CCFF]'
            : 'bg-[#E2CCFF] text-[#7A5DAC]',
          glow: 'shadow-[0_0_15px_rgba(122,93,172,0.4)]',
        },
        station: {
          icon: '🚉',
          color: isNight
            ? 'bg-[#8A8A3D] text-[#FDFD96]'
            : 'bg-[#FDFD96] text-[#8A8A3D]',
          glow: 'shadow-[0_0_15px_rgba(138,138,61,0.4)]',
        },
      };
      return (
        themeStyles[c] || {
          icon: '📍',
          color: isNight ? 'bg-[#555] text-white' : 'bg-white text-[#555]',
          glow: 'shadow-sm',
        }
      );
    }, [poi.category, isNight]);

    /**
     * [update-lifecycle-animation]
     * 매 프레임마다 카메라와의 거리를 계산하여 페이드, 미니멀 전환, 스케일 애니메이션을 수행합니다.
     * * 🚀 [Refactored]
     * 1. React State(origin prop) 대신 Store를 직접 조회하여 위치 튀는 현상 해결.
     * 2. Terrain Height를 반영하여 Y축 위치 보정.
     */
    useFrame((state, delta): void => {
      if (!groupRef.current || !containerRef.current) return;

      // 1. [Atomic-Positioning] Store에서 최신 오리진 즉시 조회 (React 렌더링 사이클 무시)
      const currentOrigin = useMapStore.getState().origin;

      // 2. [get-local-position] 실시간 로컬 좌표 계산
      const lx = poi.center.x - currentOrigin.x + VOXEL_CONFIG.HALF;
      const lz = -(poi.center.y - currentOrigin.y + VOXEL_CONFIG.HALF); // Y -> -Z (North)

      // 3. [Sync] Three.js 객체 직접 이동
      // 🎯 [Terrain-Snap] 계산된 지형 높이(terrainHeight) + 기본 오프셋(0.5m)
      groupRef.current.position.set(lx, 0.5 + terrainHeight, lz);

      // 4. 거리 계산 (이미 group 위치를 잡았으므로 카메라와의 거리는 정확함)
      const distance = state.camera.position.distanceTo(
        groupRef.current.position,
      );

      // 5. [Culling] 너무 멀면 React State 업데이트를 통해 렌더링 중단
      if (distance > CULL_DISTANCE) {
        if (!isCulled) setIsCulled(true);
        return; // 이후 로직 생략
      } else {
        if (isCulled) setIsCulled(false);
      }

      const minimalThreshold: number = FADE_START * 0.3;
      const currentMinimal: boolean = distance > minimalThreshold;
      if (currentMinimal !== lastIsMinimal.current) {
        lastIsMinimal.current = currentMinimal;
        setIsMinimal(currentMinimal);
      }

      let targetOpacity: number = 1;
      if (distance > FADE_START) {
        targetOpacity = THREE.MathUtils.mapLinear(
          distance,
          FADE_START,
          FADE_END,
          1,
          0,
        );
      }

      const finalOpacity: number =
        isOccluded || distance > FADE_END ? 0 : Math.max(0, targetOpacity);
      const isVisible: boolean = finalOpacity > 0.01;

      // Spring Physics (Stiffness 250, Damping 15)
      const targetScale: number = isVisible ? 1 : 0;
      const ax: number =
        -250 * (springValue.current - targetScale) - 15 * velocity.current;

      velocity.current += ax * delta;
      springValue.current += velocity.current * delta;

      const element: HTMLDivElement = containerRef.current;
      if (!isVisible && springValue.current < 0.001) {
        element.style.display = 'none';
      } else {
        element.style.display = 'flex';
        element.style.opacity = finalOpacity.toString();
        element.style.transform = `scale(${springValue.current})`;
      }
    });

    // 🎯 컬링된 상태면 아예 렌더링하지 않음 (React Tree에서 제외하여 성능 확보)
    if (isCulled) return null;

    return (
      <group ref={groupRef}>
        <Html
          portal={
            htmlPortalElement ? { current: htmlPortalElement } : undefined
          }
          // 🎯 Group이 이미 위치를 잡았으므로 Html은 (0,0,0)에 배치
          position={[0, 0, 0]}
          center
          occlude
          onOcclude={setIsOccluded}
          sprite
          style={{ pointerEvents: 'none' }}
          // 🎯 zIndexRange를 사용하여 거리에 따른 정렬 보장
          zIndexRange={[100, 0]}
        >
          <div
            ref={containerRef}
            className="flex flex-col items-center origin-bottom select-none transition-transform duration-300"
          >
            {/* ... (기존 JSX 구조 100% 유지) ... */}
            <div
              onClick={(e: React.MouseEvent): void => {
                e.stopPropagation();
                setSelectedPoi(poi);
              }}
              onPointerOver={(): void => setIsHovered(true)}
              onPointerOut={(): void => setIsHovered(false)}
              className={cn(
                'flex items-center rounded-full border transition-all duration-500 ease-out cursor-pointer pointer-events-auto',
                'antialiased tracking-tighter',
                isNight
                  ? 'bg-slate-900/70 backdrop-blur-md border-white/10 shadow-[0_8px_16px_rgba(0,0,0,0.4)]'
                  : 'bg-white/85 backdrop-blur-[6px] border-white/60 shadow-[0_4px_12px_rgba(0,0,0,0.08)]',
                isHovered ? '-translate-y-2' : 'translate-y-0',
                shouldShowName ? 'gap-1.5 px-2 py-1' : 'gap-0 p-1',
              )}
            >
              <div
                className={cn(
                  'w-5 h-5 flex-none flex items-center justify-center rounded-full text-[10px] transition-all duration-500',
                  categoryInfo.color,
                  isHovered && categoryInfo.glow,
                  isMinimal && !isHovered && 'scale-75',
                )}
              >
                <span className="leading-none">{categoryInfo.icon}</span>
              </div>

              <div
                className={cn(
                  'overflow-hidden transition-all duration-500 ease-in-out',
                  shouldShowName
                    ? 'max-w-[200px] opacity-100'
                    : 'max-w-0 opacity-0',
                )}
              >
                <span
                  className={cn(
                    'text-[12px] font-bold pr-1 select-none leading-none pt-[1px] block ml-1 whitespace-nowrap',
                    isNight ? 'text-white/90' : 'text-slate-800',
                  )}
                >
                  {poi.name}
                </span>
              </div>
            </div>

            <div
              className={cn(
                'w-1.5 h-1.5 mx-auto rounded-full mt-1.5 border-2 transition-all duration-500',
                isNight
                  ? 'bg-white/60 border-white/20'
                  : 'bg-slate-500 border-white',
                isMinimal && !isHovered ? 'scale-0' : 'scale-100',
                isHovered && 'scale-150 bg-blue-400 border-blue-100',
              )}
            />
          </div>
        </Html>
      </group>
    );
  },
);

PoiItem.displayName = 'PoiItem';
