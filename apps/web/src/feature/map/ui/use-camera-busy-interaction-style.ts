// @pxl/web/feature/map/ui/use-camera-busy-interaction-style.ts

import { cn } from "@/shared/lib/util/cn.ts";
import { useKernel } from "@/kernel/shell.ts";

/**
 * The standard "dim and stop responding to touch while the camera is busy"
 * treatment for map overlay chrome (top-overlays.tsx, bottom-overlays.tsx).
 * Both also gate a nested element's own pointer-events on the raw boolean
 * directly, so it's returned alongside the className instead of folded away.
 */
export function useCameraBusyInteractionStyle(): { isCameraBusy: boolean; interactionStyle: string } {
  const isCameraBusy = useKernel((s) => s.camera_busy);

  const interactionStyle = cn(
    // Not transition-all: these containers position from --vv-top/--vv-bottom,
    // and `all` swept top/bottom into the same 500ms ease, so an overlay
    // visibly eased toward where the keyboard had been half a second ago.
    "transition-[opacity,transform,filter] duration-500 ease-in-out",
    isCameraBusy ? "pointer-events-none opacity-40 scale-[0.98] blur-[0.5px]" : "opacity-100 scale-100 blur-0",
  );

  return { isCameraBusy, interactionStyle };
}
