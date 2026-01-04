import React, { useEffect, useRef } from 'react';
import { useMapStore } from '@/entities/geo/model/map-store';
import { useMapNavigation } from '@/entities/geo/model/use-map-navigation';
import { cn } from '@/shared/lib/utils';

/**
 * [map-html-overlay]
 * Three.js 캔버스 위에 겹쳐지는(Overlay) HTML 전용 레이어입니다.
 * <Html portal={...}> 컴포넌트들이 실제 마운트되는 DOM 위치(Root) 역할을 합니다.
 */
export const MapHtmlOverlay = React.memo(() => {
  const overlayRef = useRef<HTMLDivElement>(null);
  const setHtmlPortalElement = useMapStore(
    (state) => state.setHtmlPortalElement,
  );

  // 🎯 [Theme-Sync] UI 테마도 3D 월드 테마와 일치시킵니다.
  const { themeKey } = useMapNavigation();

  /**
   * [sync-dom-ref]
   * 이 DOM 요소를 스토어에 등록하여,
   * Canvas 내부의 객체들이 이곳을 타겟으로 HTML을 렌더링할 수 있게 합니다.
   */
  useEffect(() => {
    if (overlayRef.current) {
      setHtmlPortalElement(overlayRef.current);
    }
    return () => setHtmlPortalElement(null);
  }, [setHtmlPortalElement]);

  return (
    <div
      ref={overlayRef}
      id="map-html-overlay"
      /**
       * pointer-events-none:
       * 기본적으로 클릭을 투과시켜 아래의 3D 캔버스를 조작할 수 있게 합니다.
       * (내부의 버튼 등 상호작용이 필요한 요소는 pointer-events-auto로 개별 설정해야 함)
       */
      className={cn(
        `theme-${themeKey}`,
        'absolute inset-0 z-[50] overflow-hidden select-none pointer-events-none',
      )}
    />
  );
});

MapHtmlOverlay.displayName = 'MapHtmlOverlay';
