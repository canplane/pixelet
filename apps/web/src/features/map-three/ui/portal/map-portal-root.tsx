import React, { useEffect, useRef } from 'react';
import { useMapStore } from '../../model/map-store'; // 경로 확인 필요
import { cn } from '@/shared/lib/utils';

export const MapPortalRoot = () => {
  const portalRef = useRef<HTMLDivElement>(null);
  const setPortalElement = useMapStore((state) => state.setPortalElement);

  useEffect(() => {
    // 컴포넌트가 마운트될 때 실제 DOM 노드를 Store에 저장합니다.
    if (portalRef.current) {
      setPortalElement(portalRef.current);
    }

    // 언마운트 시 정리 (선택 사항)
    return () => setPortalElement(null);
  }, [setPortalElement]);

  return (
    <div
      ref={portalRef}
      id="map-portal-root"
      /* z-[50]: POI 뱃지들이 노는 층
         pointer-events-none: 아래에 있는 캔버스의 클릭을 방해하지 않음
      */
      className={cn(
        'theme-cozy',
        'absolute inset-0 pointer-events-none z-[50] overflow-hidden',
      )}
    />
  );
};
