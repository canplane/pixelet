// @pxl/web/modules/map/scene.tsx

import { useEffect } from "react";

import { get_theme_key, themes, type WorldTheme } from "@/modules/world/render/themes.ts";

import { useMapBootstrap } from "./navigation/use-map-bootstrap.ts";
import { useTileFetcher } from "@/modules/world/terrain/use-tile-fetcher.ts";
import { MapOverlays } from "./ui/overlay/map-overlays.tsx";
import { MapHTMLPortal } from "./portal/html-portal.tsx";
import { WorldExportTools } from "@/modules/world/terrain/ui/export-tools.tsx";
import { MapToastController } from "./ui/toast/toast-controller.tsx";
import { MapCanvasHost } from "./render/host.tsx";

export function MapScene() {
  useMapBootstrap();
  useTileFetcher();

  const theme_key = get_theme_key();
  const theme: WorldTheme = themes[theme_key];

  /**
   * Prevent browser touch gestures from competing with the map input layer.
   * Canvas ownership is now externalized, so the shell should own DOM policy here.
   */
  useEffect(() => {
    document.body.style.touchAction = "none";
    return () => {
      document.body.style.touchAction = "";
    };
  }, []);

  return (
    <div className="fixed inset-0 overflow-hidden bg-black">
      <MapToastController />

      {/* Theme background layer matched to fog color to hide the 2D/3D seam. */}
      <div
        className="absolute inset-0 transition-colors duration-1500 ease-in-out"
        style={{
          backgroundColor: theme.fog,
        }}
      >
        <MapCanvasHost />
      </div>

      {/* HTML overlay portal */}
      <MapHTMLPortal />

      {/* Absolute-position UI layer */}
      <MapOverlays />

      {/* Development-only terrain export tools */}
      {__IS_DEV__ && <WorldExportTools />}
    </div>
  );
}
