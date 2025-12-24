import React, { useMemo, useRef, useState, type ReactElement } from 'react'; // ReactElement 추가
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import { Html } from '@react-three/drei';
import { useMapStore } from '../../model/map-store';
import { useMapNavigation } from '../../model/use-map-navigation';
import { useRelativePosition } from '../../model/use-relative-position';
import type { MapPoi } from '../../../../entities/osm/lib/osm-parser';
import { THREE_CONFIG } from '../../model/three-config';
import { cn } from '@/shared/lib/utils';
import { VOXEL_CONFIG } from '../../model/voxel-config';
import type { MeterPos } from '@/shared/types/map';

const { FADE_START, FADE_END } = THREE_CONFIG.RENDERING;

interface OsmPoiItemProps {
  poi: MapPoi;
}

/**
 * [osm-poi-item]
 * 절대 좌표 기반의 POI 데이터를 Three.js 씬 내 상대 좌표로 변환하여 렌더링합니다.
 * 거리 기반의 LOD(Level of Detail)와 스프링 애니메이션을 지원하며,
 * 빌보드(Billboard) 기법을 통해 항상 카메라를 정면으로 응시합니다.
 */
export const OsmPoiItem = React.memo(
  ({ poi }: OsmPoiItemProps): ReactElement => {
    // JSX.Element 대신 ReactElement 사용
    const portalElement: HTMLElement | null = useMapStore(
      (state) => state.portalElement,
    );
    const setSelectedPoi = useMapStore((state) => state.setSelectedPoi);

    const { themeKey } = useMapNavigation();
    const isNight: boolean = themeKey === 'night';

    const containerRef = useRef<HTMLDivElement>(null);
    const [isHovered, setIsHovered] = useState<boolean>(false);
    const [isOccluded, setIsOccluded] = useState<boolean>(false);

    /**
     * [get-local-position]
     * 📍 [floating-origin-apply]
     * Floating Origin 지침에 따라 절대 미터 좌표를 현재 씬의 로컬 좌표로 변환합니다.
     * 지면(Voxel Grid) 위에 배치하기 위해 Y축 오프셋을 적용합니다.
     */
    const relativePosArray: MeterPos = useRelativePosition(poi.center);
    const localPosition: THREE.Vector3 = useMemo((): THREE.Vector3 => {
      return new THREE.Vector3(
        relativePosArray.x,
        VOXEL_CONFIG.HALF + 0.5,
        relativePosArray.z,
      );
    }, [relativePosArray]);

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

    const springValue = useRef<number>(0);
    const velocity = useRef<number>(0);
    const [isMinimal, setIsMinimal] = useState<boolean>(false);
    const lastIsMinimal = useRef<boolean>(false);

    /**
     * [label-visibility-logic]
     * 이름표의 노출 여부를 결정합니다.
     */
    const shouldShowName: boolean = !isMinimal || isHovered;

    /**
     * [update-lifecycle-animation]
     * 매 프레임마다 카메라와의 거리를 계산하여 페이드, 미니멀 전환, 스케일 애니메이션을 수행합니다.
     */
    useFrame((state: { camera: THREE.Camera }, delta: number): void => {
      if (!containerRef.current) return;

      const distance: number = state.camera.position.distanceTo(localPosition);
      const element: HTMLDivElement = containerRef.current;

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

      /**
       * [spring-physics-tuning]
       * 쫀득한 느낌을 위한 파라미터 조절:
       * - Stiffness (250): 더 빠르게 목표치로 수렴 (기존 180)
       * - Damping (15): 진동을 적절히 잡아주어 '찰랑'거리는 끝맛 유지 (기존 12)
       */
      const targetScale: number = isVisible ? 1 : 0;

      // 가속도 계산: -Stiffness * (현재 - 목표) - Damping * 속도
      const ax: number =
        -250 * (springValue.current - targetScale) - 15 * velocity.current;

      velocity.current += ax * delta;
      springValue.current += velocity.current * delta;

      // 렌더링 최적화 및 스타일 적용
      if (!isVisible && springValue.current < 0.001) {
        element.style.display = 'none';
      } else {
        element.style.display = 'flex';
        element.style.opacity = finalOpacity.toString();
        // 쫀득함을 극대화하기 위해 약간의 Overshoot(1.0을 살짝 넘음)이 허용됩니다.
        element.style.transform = `scale(${springValue.current})`;
      }
    });

    return (
      <Html
        portal={portalElement ? { current: portalElement } : undefined}
        position={localPosition}
        center
        occlude
        onOcclude={setIsOccluded}
        sprite
        style={{ pointerEvents: 'none' }}
      >
        <div
          ref={containerRef}
          className="flex flex-col items-center origin-bottom select-none transition-transform duration-300"
        >
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
    );
  },
);

OsmPoiItem.displayName = 'OsmPoiItem';
