// @pxl/web/modules/map/ui/overlay/bottom_overlays.tsx

import { cn } from "@/shared/lib/utils/cn.ts";
import { CameraStatus, useCameraStore } from "@/modules/camera/store.ts";
import { ModeBar } from "./mode_bar.tsx";

export function BottomOverlays() {
  const camera_status: CameraStatus = useCameraStore((state) => state.camera_status);
  const is_camera_busy: boolean = camera_status !== CameraStatus.IDLE;

  const interaction_style = cn(
    // Not transition-all: these containers position from --vv-top/--vv-bottom,
    // and `all` swept top/bottom into the same 500ms ease, so an overlay
    // visibly eased toward where the keyboard had been half a second ago.
    "transition-[opacity,transform,filter] duration-500 ease-in-out",
    is_camera_busy ? "pointer-events-none opacity-40 scale-[0.98] blur-[0.5px]" : "opacity-100 scale-100 blur-0",
  );

  return (
    <>
      <div className={cn("absolute bottom-8 left-8 z-50", interaction_style)} />

      {/* Keep this bottom offset in sync with MODE_BAR_BOTTOM_OFFSET_PX (overlay_safe_area.ts) — toaster.tsx's own safe margin depends on it. */}
      <div
        // iOS delivers visualViewport events inconsistently (streamed while the
        // keyboard opens, or as one final value if it was already up), so only
        // `bottom` gets a short ease here — long enough to read as motion, short
        // enough to stay attached during the streaming case.
        className={cn("absolute bottom-[calc(1rem+var(--vv-bottom,0px))] left-4 right-4 z-40 pointer-events-none transition-[bottom] duration-150 ease-out", interaction_style)}
      >
        {/* ModeBar's own w-full/flex-1 chain needs a real ancestor width to resolve against — without this wrapper, flex-grow has nothing to grow into. */}
        <div className={cn("min-w-0 w-full flex justify-center", !is_camera_busy && "pointer-events-auto")}>
          <ModeBar />
        </div>
      </div>
    </>
  );
}
