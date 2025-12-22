import React, { useMemo, useRef, useState } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import { Html } from '@react-three/drei';
import { useMapStore } from '../../model/map-store';
import type { MapPoi } from '../../lib/osm-parser';
import { THREE_CONFIG } from '../../model/three-config';
import { cn } from '@/shared/lib/utils';

const { FADE_START, FADE_END } = THREE_CONFIG.RENDERING;

/**
 * 관심 지점(POI)을 지도 위에 HTML 라벨(Badge)로 렌더링하는 컴포넌트
 */
export const OsmPoiItem = React.memo(({ poi }: { poi: MapPoi }) => {
  const portalElement = useMapStore((state) => state.portalElement);
  const setSelectedPoi = useMapStore((state) => state.setSelectedPoi);

  // 🚀 [Floating Origin] 누적 오프셋 구독
  const originOffset = useMapStore((state) => state.originOffset);

  const containerRef = useRef<HTMLDivElement>(null);
  const [isHovered, setIsHovered] = useState(false);
  const [isOccluded, setIsOccluded] = useState(false);

  // 1. 동화 느낌의 포근한 파스텔 컬러 팔레트
  const categoryInfo = useMemo(() => {
    const c = poi.category;
    // 부드러운 파스텔 톤으로 전면 수정
    if (c === 'cafe')
      return { icon: '☕', color: 'bg-[#FFD1B3] text-[#A35D36]' }; // 살구
    if (c === 'restaurant')
      return { icon: '🍴', color: 'bg-[#FFB7B2] text-[#B04A43]' }; // 핑크
    if (c === 'shop' || c === 'mall')
      return { icon: '🛍️', color: 'bg-[#B2CEFE] text-[#4A6DAB]' }; // 하늘
    if (c === 'tourism' || c === 'historic')
      return { icon: '🏛️', color: 'bg-[#E2CCFF] text-[#7A5DAC]' }; // 연보라
    if (c === 'station')
      return { icon: '🚉', color: 'bg-[#FDFD96] text-[#8A8A3D]' }; // 파스텔 노랑
    return { icon: '📍', color: 'bg-[#D1D1D1] text-[#666666]' }; // 연그레이
  }, [poi.category]);

  /**
   * 1. 로컬 기반의 라벨 위치 계산
   * 🚀 [보정] 절대 좌표(poi.center) - 누적 오프셋(originOffset)
   */
  const localLabelPosition = useMemo(() => {
    return new THREE.Vector3(
      poi.center.x - originOffset.x,
      2, // 지면에서 2m 띄움
      poi.center.z - originOffset.z,
    );
  }, [poi.center, originOffset]); // 🚀 오프셋이 변하면 위치도 재계산

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
      position={localLabelPosition} // 🚀 보정된 로컬 위치 전달
      center
      occlude
      onOcclude={setIsOccluded}
      sprite // 🚀 왜곡 방지를 위해 sprite 추가
      style={{ pointerEvents: 'none' }}
    >
      {/* 🚀 1층 시스템 레이어: 여기서 강제로 '너비 확보'와 '줄바꿈 금지'를 겁니다. */}
      <div
        ref={containerRef}
        className="flex flex-col items-center origin-bottom"
        style={{
          whiteSpace: 'nowrap',
          minWidth: 'max-content',
          // 🚀 최적화 4: transform에는 transition을 절대 걸지 마세요! (useFrame과 충돌)
          // opacity만 CSS transition을 타게 하거나, 아예 뺍니다.
          transition: 'opacity 0.5s ease-in-out',
        }}
      >
        <div
          onClick={() => setSelectedPoi(poi)}
          onPointerOver={() => setIsHovered(true)}
          onPointerOut={() => setIsHovered(false)}
          className={cn(
            'flex items-center bg-white/90 backdrop-blur-[2px] rounded-full shadow-sm border border-white/50 transition-all duration-500 ease-in-out',
            'antialiased tracking-tighter cursor-pointer pointer-events-auto',
            // 🚀 미니멀 모드일 때와 아닐 때의 패딩/간격 전환
            isMinimal ? 'gap-0 px-1 py-1' : 'gap-1.5 px-2 py-1',
            isHovered ? '-translate-y-1 shadow-md bg-white' : 'translate-y-0',
          )}
        >
          {/* 파스텔 아이콘 */}
          <div
            className={cn(
              'w-4 h-4 flex-none flex items-center justify-center rounded-full text-[9px] transition-transform duration-500',
              categoryInfo.color,
              isMinimal && 'scale-75',
            )}
          >
            <span className="leading-none transform scale-90">
              {categoryInfo.icon}
            </span>
          </div>

          {/* 텍스트 영역: width 전환을 부드럽게 */}
          <div
            className={cn(
              'overflow-hidden transition-all duration-500 ease-in-out',
              // 🚀 핵심: width 대신 max-width를 사용합니다.
              // 미니멀 모드일 땐 0, 아닐 땐 충분히 큰 값을 주어 유연하게 늘어나게 합니다.
              isMinimal ? 'max-w-0 opacity-0' : 'max-w-[200px] opacity-100',
            )}
          >
            <span
              className={cn(
                'text-[12px] font-bold text-slate-700/90 pr-0.5 select-none leading-none pt-[1px] block ml-1 whitespace-nowrap',
                'pixel-antialiasing-off',
              )}
            >
              {poi.name}
            </span>
          </div>
        </div>

        {/* 앵커 점 */}
        <div
          className={cn(
            'w-1 h-1 bg-white/90 mx-auto rounded-full mt-0.5 border border-white/50 shadow-sm shrink-0 transition-opacity duration-500',
            isMinimal ? 'opacity-0' : 'opacity-100',
          )}
        />
      </div>
    </Html>
  );
});
