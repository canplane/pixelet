// @pxl/web/module/map/portal/html_portal.tsx

import { memo, useEffect, useRef } from "react";

import { useMapStore } from "@/module/map/store.ts";
import { useThemeKey } from "@/module/world/environment/theme_clock.ts";

import { cn } from "@/shared/lib/util/cn.ts";

/**
 * HTML-only overlay layer stacked above the Three.js canvas.
 * Acts as the DOM root that <Html portal={...}> components actually mount into.
 */
export const MapHTMLPortal = memo(() => {
  const set_html_portal_el = useMapStore.getState().set_html_portal_el;

  const theme_key = useThemeKey();

  const overlay_ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (overlay_ref.current) set_html_portal_el(overlay_ref.current);
    return () => set_html_portal_el(null);
  }, []);

  return (
    <div
      ref={overlay_ref}
      id="map-html_portal"
      /**
       * pointer-events-none: lets clicks pass through by default, so the 3D
       * canvas underneath stays interactive. (Elements that need real
       * interaction, e.g. buttons, must opt back in individually with
       * pointer-events-auto.)
       */
      className={cn(
        `theme-${theme_key}`,
        "absolute inset-0 z-50 overflow-hidden select-none pointer-events-none",
      )}
    />
  );
});
MapHTMLPortal.displayName = "MapHTMLPortal";
