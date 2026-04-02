// @pxl/web/modules/map/portal/html-portal.tsx

import { memo, useEffect, useRef } from "react";

import { useMapStore } from "@/modules/map/navigation/map-store.ts";
import { get_theme_key } from "@/modules/world/render/themes.ts";

import { cn } from "@/shared/lib/utils/cn.ts";

/**
 * Three.js 캔버스 위에 겹쳐지는(Overlay) HTML 전용 레이어입니다.
 * <Html portal={...}> 컴포넌트들이 실제 마운트되는 DOM 위치(Root) 역할을 합니다.
 */
export const MapHTMLPortal = memo(() => {
  const set_html_portal_el = useMapStore.getState().set_html_portal_el;

  const theme_key = get_theme_key();

  const overlay_ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (overlay_ref.current) set_html_portal_el(overlay_ref.current);
    return () => set_html_portal_el(null);
  }, []);

  return (
    <div
      ref={overlay_ref}
      id="map-html-portal"
      /**
       * pointer-events-none:
       * 기본적으로 클릭을 투과시켜 아래의 3D 캔버스를 조작할 수 있게 합니다.
       * (내부의 버튼 등 상호작용이 필요한 요소는 pointer-events-auto로 개별 설정해야 함)
       */
      className={cn(
        `theme-${theme_key}`,
        "absolute inset-0 z-50 overflow-hidden select-none pointer-events-none",
      )}
    />
  );
});
MapHTMLPortal.displayName = "MapHTMLPortal";
