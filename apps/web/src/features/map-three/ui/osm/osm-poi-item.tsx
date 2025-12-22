import React, { useMemo, useRef, useState } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import { Html } from '@react-three/drei';
import { useMapStore } from '../../model/map-store';
import { useMapNavigation } from '../../model/use-map-navigation';
import type { MapPoi } from '../../lib/osm-parser';
import { THREE_CONFIG } from '../../model/three-config';
import { cn } from '@/shared/lib/utils';
import { projection } from '@/shared/lib/geo/projection'; // 격자 유틸리티 임포트

const { HALF_GRID } = projection; // 2m 보정용

const { FADE_START, FADE_END } = THREE_CONFIG.RENDERING;

/**
 * 관심 지점(POI)을 지도 위에 HTML 라벨(Badge)로 렌더링하는 컴포넌트
 */
export const OsmPoiItem = React.memo(({ poi }: { poi: MapPoi }) => {
  const portalElement = useMapStore((state) => state.portalElement);
  const setSelectedPoi = useMapStore((state) => state.setSelectedPoi);

  // 1. 내비게이션 엔진에서 상태 추출
  const { originOffset, themeKey } = useMapNavigation();
  const isNight = themeKey === 'night';

  const containerRef = useRef<HTMLDivElement>(null);
  const [isHovered, setIsHovered] = useState(false);
  const [isOccluded, setIsOccluded] = useState(false);

  // 2. [테마 대응] 카테고리별 컬러 팔레트 (낮/밤 반전 로직)
  const categoryInfo = useMemo(() => {
    const c = poi.category;

    const themeStyles: Record<
      string,
      { icon: string; color: string; glow: string }
    > = {
      cafe: {
        icon: '☕',
        color: isNight
          ? 'bg-[#A35D36] text-[#FFD1B3]'
          : 'bg-[#FFD1B3] text-[#A35D36]',
        glow: 'shadow-[0_0_12px_rgba(255,209,179,0.5)]',
      },
      restaurant: {
        icon: '🍴',
        color: isNight
          ? 'bg-[#B04A43] text-[#FFB7B2]'
          : 'bg-[#FFB7B2] text-[#B04A43]',
        glow: 'shadow-[0_0_12px_rgba(255,183,178,0.5)]',
      },
      shop: {
        icon: '🛍️',
        color: isNight
          ? 'bg-[#4A6DAB] text-[#B2CEFE]'
          : 'bg-[#B2CEFE] text-[#4A6DAB]',
        glow: 'shadow-[0_0_12px_rgba(178,206,254,0.5)]',
      },
      tourism: {
        icon: '🏛️',
        color: isNight
          ? 'bg-[#7A5DAC] text-[#E2CCFF]'
          : 'bg-[#E2CCFF] text-[#7A5DAC]',
        glow: 'shadow-[0_0_12px_rgba(226,204,255,0.5)]',
      },
      station: {
        icon: '🚉',
        color: isNight
          ? 'bg-[#8A8A3D] text-[#FDFD96]'
          : 'bg-[#FDFD96] text-[#8A8A3D]',
        glow: 'shadow-[0_0_12px_rgba(253,253,150,0.5)]',
      },
    };

    return (
      themeStyles[c] || {
        icon: '📍',
        color: isNight
          ? 'bg-[#666666] text-[#D1D1D1]'
          : 'bg-[#D1D1D1] text-[#666666]',
        glow: 'shadow-sm',
      }
    );
  }, [poi.category, isNight]);

  // 3. [격자 정렬] 4m 스냅 + 2m 중앙 보정
  const localLabelPosition = useMemo(() => {
    const snappedAbsPos = projection.snapToGrid(poi.center);
    return new THREE.Vector3(
      snappedAbsPos.x + HALF_GRID - originOffset.x,
      2.5, // 0.5m 더 높여서 시인성 확보
      snappedAbsPos.z + HALF_GRID - originOffset.z,
    );
  }, [poi.center, originOffset]);

  // 🚀 탄성 애니메이션을 위한 ref
  const springValue = useRef(0); // 현재 스케일 값 (0 ~ 1)
  const velocity = useRef(0); // 속도

  const [isMinimal, setIsMinimal] = useState(false);

  // 🚀 최적화 1: 이전 상태를 저장해서 불필요한 setState 방지
  const lastIsMinimal = useRef(false);

  /**
   * 통합 렌더링 루프
   */
  useFrame((state, delta) => {
    if (!containerRef.current) return;

    // 🚀 [보정] localLabelPosition은 이미 originOffset이 적용된 상태이므로 정확합니다.
    const distance = state.camera.position.distanceTo(localLabelPosition);
    const element = containerRef.current;

    // 1. Minimal 모드 전환 (LOD)
    // 🚀 팁: 미니멀 전환 거리를 안개 시작 지점의 약 1/4 정도로 잡으면 자연스럽습니다.
    const minimalThreshold = FADE_START * 0.25;
    const currentMinimal = distance > minimalThreshold;
    if (currentMinimal !== lastIsMinimal.current) {
      lastIsMinimal.current = currentMinimal;
      setIsMinimal(currentMinimal);
    }

    // 2. 투명도 계산 (안개와 동기화)
    let targetOpacity = 1;
    if (distance > FADE_START) {
      targetOpacity = THREE.MathUtils.mapLinear(
        distance,
        FADE_START,
        FADE_END,
        1,
        0,
      );
    }

    // 안개 끝 지점(FADE_END)을 넘어가면 강제로 0
    const finalOpacity =
      isOccluded || distance > FADE_END ? 0 : Math.max(0, targetOpacity);
    const isVisible = finalOpacity > 0.01;

    // 3. Spring Physics (나타나고 사라질 때 팅기는 효과)
    const targetScale = isVisible ? 1 : 0;
    const ax =
      -180 * (springValue.current - targetScale) - 12 * velocity.current;
    velocity.current += ax * delta;
    springValue.current += velocity.current * delta;

    // 4. 성능 최적화: DOM 조작
    if (!isVisible && springValue.current < 0.001) {
      element.style.display = 'none';
    } else {
      element.style.display = 'flex';
      element.style.opacity = finalOpacity.toString();
      element.style.transform = `scale(${springValue.current})`;
    }
  });

  return (
    <Html
      portal={portalElement ? { current: portalElement } : undefined}
      position={localLabelPosition}
      center
      occlude
      onOcclude={setIsOccluded}
      sprite
      style={{ pointerEvents: 'none' }}
    >
      <div
        ref={containerRef}
        className="flex flex-col items-center origin-bottom select-none"
        style={{ whiteSpace: 'nowrap', minWidth: 'max-content' }}
      >
        {/* 배지 본체: Glassmorphism + Glow */}
        <div
          onClick={() => setSelectedPoi(poi)}
          onPointerOver={() => setIsHovered(true)}
          onPointerOut={() => setIsHovered(false)}
          className={cn(
            'flex items-center rounded-full border transition-all duration-500 ease-out cursor-pointer pointer-events-auto',
            'antialiased tracking-tighter',
            // 🌙 밤/낮 베이스 스타일 (location-control 효과 전파)
            isNight
              ? 'bg-slate-900/60 backdrop-blur-md border-white/10 shadow-[0_4px_12px_rgba(0,0,0,0.3)]'
              : 'bg-white/80 backdrop-blur-[4px] border-white/60 shadow-[0_2px_8px_rgba(0,0,0,0.05)]',
            // ✨ 호버 및 상태별 변화
            isHovered ? '-translate-y-1 scale-105' : 'translate-y-0 scale-100',
            isHovered && isNight ? 'border-white/30 bg-slate-800/80' : '',
            isMinimal ? 'gap-0 px-1 py-1' : 'gap-1.5 px-2 py-1',
          )}
        >
          {/* 아이콘 서클 + 글로우 */}
          <div
            className={cn(
              'w-5 h-5 flex-none flex items-center justify-center rounded-full text-[10px] transition-all duration-500',
              categoryInfo.color,
              isHovered && categoryInfo.glow, // 호버 시 카테고리 고유색 글로우
              isMinimal && 'scale-75',
            )}
          >
            <span className="leading-none">{categoryInfo.icon}</span>
          </div>

          {/* 텍스트 영역 */}
          <div
            className={cn(
              'overflow-hidden transition-all duration-500 ease-in-out',
              isMinimal ? 'max-w-0 opacity-0' : 'max-w-[200px] opacity-100',
            )}
          >
            <span
              className={cn(
                'text-[12px] font-bold pr-1 select-none leading-none pt-[1px] block ml-1',
                isNight ? 'text-slate-100/90' : 'text-slate-700/90',
              )}
            >
              {poi.name}
            </span>
          </div>
        </div>

        {/* 앵커 핀 (바닥 점) */}
        <div
          className={cn(
            'w-1.5 h-1.5 mx-auto rounded-full mt-1 border transition-all duration-500',
            isNight
              ? 'bg-white/40 border-white/20'
              : 'bg-slate-400 border-white',
            isMinimal ? 'opacity-0 scale-50' : 'opacity-100 scale-100',
            isHovered && 'scale-125 bg-red-400',
          )}
        />
      </div>
    </Html>
  );
});
