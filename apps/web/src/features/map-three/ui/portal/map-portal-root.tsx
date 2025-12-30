import React, { useEffect, useRef } from 'react';
import { useMapStore } from '@/entities/osm/model/map-store';
import { cn } from '@/shared/lib/utils';

/**
 * [map-portal-root]
 * Three.js 캔버스 위에 HTML 기반 오버레이(POI 뱃지 등)를 렌더링하기 위한 전용 포탈 컨테이너입니다.
 */
export const MapPortalRoot = React.memo(() => {
  const portalRef = useRef<HTMLDivElement>(null);
  const setPortalElement = useMapStore((state) => state.setPortalElement);

  /**
   * [sync-portal-ref]
   * 실제 DOM 노드를 글로벌 스토어에 등록하여 다른 컴포넌트들이 Html(drei) 포탈로 접근할 수 있게 합니다.
   */
  useEffect(() => {
    if (portalRef.current) {
      setPortalElement(portalRef.current);
    }
    return () => setPortalElement(null);
  }, [setPortalElement]);

  return (
    <div
      ref={portalRef}
      id="map-portal-root"
      /**
       * z-[50]: UI 오버레이와 3D 캔버스 사이의 중간 레이어 층을 형성합니다.
       * pointer-events-none: 하위의 캔버스 조작을 방해하지 않도록 설정합니다.
       * overflow-hidden: 렌더링 영역을 벗어나는 자식 요소로 인한 스크롤 발생을 차단합니다.
       */
      className={cn(
        'theme-cozy',
        'absolute inset-0 pointer-events-none z-[50] overflow-hidden select-none',
      )}
    />
  );
});
