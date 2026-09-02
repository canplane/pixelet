// @pxl/web/modules/map/ui/overlay/bottom_overlays.tsx

import { cn } from "@/shared/lib/utils/cn.ts";
import { use_camera_busy_interaction_style } from "@/modules/camera/use_camera_busy_interaction_style.ts";
import { ModeBar } from "./mode_bar.tsx";

// How far off the screen bottom this positions ModeBar — app_layout.tsx
// reads this (together with mode_bar.tsx's own MODE_BAR_HEIGHT_PX) to keep
// Toaster clear of it.
export const BOTTOM_OFFSET_PX = 16;

export function BottomOverlays() {
  const { is_camera_busy, interaction_style } = use_camera_busy_interaction_style();

  return (
    <>
      <div className={cn("absolute bottom-8 left-8 z-50", interaction_style)} />

      <div
        // iOS delivers visualViewport events inconsistently (streamed while the
        // keyboard opens, or as one final value if it was already up), so only
        // `bottom` gets a short ease here — long enough to read as motion, short
        // enough to stay attached during the streaming case.
        //
        // bottom via inline style, not a Tailwind bottom-[calc(...)] class:
        // BOTTOM_OFFSET_PX needs to reach here as a real value, and
        // Tailwind's JIT scanner can't see a variable interpolated into an
        // arbitrary-value class (see mode_bar.tsx's own note on this).
        className={cn("absolute left-4 right-4 z-40 pointer-events-none transition-[bottom] duration-150 ease-out", interaction_style)}
        style={{ bottom: `calc(${BOTTOM_OFFSET_PX}px + var(--vv-bottom, 0px))` }}
      >
        {/* ModeBar's own w-full/flex-1 chain needs a real ancestor width to resolve against — without this wrapper, flex-grow has nothing to grow into. */}
        <div className={cn("min-w-0 w-full flex justify-center", !is_camera_busy && "pointer-events-auto")}>
          <ModeBar />
        </div>
      </div>
    </>
  );
}
