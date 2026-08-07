// @pxl/web/modules/map/ui/overlay/bottom_overlays.tsx

import { cn } from "@/shared/lib/utils/cn.ts";
import { CameraStatus, useCameraStore } from "@/modules/camera/store.ts";
import { PaletteBar } from "./palette_bar.tsx";

/**
 * Everything anchored to the bottom edge of the map canvas — split out of
 * map_overlays.tsx. The compass/scale_bar/GPS cluster that used to live
 * here moved to top_overlays.tsx (top-right) so the bottom edge stays
 * clear for the floating palette bar, which is planned to grow into this
 * app's main control surface (search, text mode, ...) — see
 * top_overlays.tsx's doc comment and overlay_safe_area.ts for the
 * reasoning/shared footprint constants that decision feeds into (the toast
 * container in sonner.tsx still reads BOTTOM_OVERLAY_SAFE_PX to clear the
 * palette bar).
 */
export function BottomOverlays() {
  const camera_status: CameraStatus = useCameraStore((state) => state.camera_status);
  const is_camera_busy: boolean = camera_status !== CameraStatus.IDLE;

  const interaction_style = cn(
    "transition-all duration-500 ease-in-out",
    is_camera_busy ? "pointer-events-none opacity-40 scale-[0.98] blur-[0.5px]" : "opacity-100 scale-100 blur-0",
  );

  return (
    <>
      {/* 4. bottom-left: origin-jump debug utility */}
      <div className={cn("absolute bottom-8 left-8 z-50", interaction_style)} />

      {
        /* OSM attribution — used to only show alongside the reverse-geocoded
      (Nominatim) label, but what this attribution actually covers is the
      whole terrain/road/building bake (this project's core data source), so
      it should stay visible whenever the map is up, regardless of whether
      the Nominatim lookup succeeds. Matches the top-4 right-4 (top-right
      address menu)'s own margin for symmetry. */
      }
      <div
        className={cn(
          "absolute bottom-3 right-3 z-50 text-[8px] text-white/30 pointer-events-none",
          interaction_style,
        )}
      >
        © OpenStreetMap contributors
      </div>

      {
        /* 5. bottom-center: floating palette bar (concept). z-40 — the toast
      pill (Sonner) uses its own portal at a much higher z-index, so it
      floats above this; sonner.tsx keeps its own margin via
      BOTTOM_OVERLAY_SAFE_PX so the two don't overlap. Keep this bottom
      value in sync with PALETTE_BAR_BOTTOM_OFFSET_PX (overlay_safe_area.ts). */
      }
      <div
        className={cn(
          "absolute bottom-6 left-1/2 -translate-x-1/2 z-40 pointer-events-none",
          interaction_style,
        )}
      >
        <div className={cn(!is_camera_busy && "pointer-events-auto")}>
          <PaletteBar />
        </div>
      </div>
    </>
  );
}
