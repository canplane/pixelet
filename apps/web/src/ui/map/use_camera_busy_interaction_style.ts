// @pxl/web/ui/map/use_camera_busy_interaction_style.ts

import { cn } from "@/kit/lib/util/cn.ts";
import { useKernel } from "@/kernel/kernel.ts";

/**
 * The standard "dim and stop responding to touch while the camera is busy"
 * treatment for map overlay chrome (top_overlays.tsx, bottom_overlays.tsx).
 * Both also gate a nested element's own pointer-events on the raw boolean
 * directly, so it's returned alongside the className instead of folded away.
 */
export function use_camera_busy_interaction_style(): { is_camera_busy: boolean; interaction_style: string } {
  const is_camera_busy = useKernel((s) => s.camera_busy);

  const interaction_style = cn(
    // Not transition-all: these containers position from --vv-top/--vv-bottom,
    // and `all` swept top/bottom into the same 500ms ease, so an overlay
    // visibly eased toward where the keyboard had been half a second ago.
    "transition-[opacity,transform,filter] duration-500 ease-in-out",
    is_camera_busy ? "pointer-events-none opacity-40 scale-[0.98] blur-[0.5px]" : "opacity-100 scale-100 blur-0",
  );

  return { is_camera_busy, interaction_style };
}
