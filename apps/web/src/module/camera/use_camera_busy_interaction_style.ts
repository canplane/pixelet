// @pxl/web/module/camera/use_camera_busy_interaction_style.ts

import { cn } from "@/shared/lib/util/cn.ts";
import { CameraStatus, useCameraStore } from "./store.ts";

/**
 * The standard "dim and stop responding to touch while the camera is busy"
 * treatment for map overlay chrome (top_overlays.tsx, bottom_overlays.tsx) —
 * pulled out of both since they had it copy-pasted identically. Both also
 * gate a nested element's own pointer-events on the raw boolean directly
 * (rather than through this className), so it's returned alongside the
 * className instead of folded away.
 */
export function use_camera_busy_interaction_style(): { is_camera_busy: boolean; interaction_style: string } {
  const camera_status = useCameraStore((state) => state.camera_status);
  const is_camera_busy = camera_status !== CameraStatus.IDLE;

  const interaction_style = cn(
    // Not transition-all: these containers position from --vv-top/--vv-bottom,
    // and `all` swept top/bottom into the same 500ms ease, so an overlay
    // visibly eased toward where the keyboard had been half a second ago.
    "transition-[opacity,transform,filter] duration-500 ease-in-out",
    is_camera_busy ? "pointer-events-none opacity-40 scale-[0.98] blur-[0.5px]" : "opacity-100 scale-100 blur-0",
  );

  return { is_camera_busy, interaction_style };
}
