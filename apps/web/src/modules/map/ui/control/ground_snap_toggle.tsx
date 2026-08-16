// @pxl/web/modules/map/ui/control/ground_snap_toggle.tsx

import { Mountain } from "lucide-react";

import { useCameraStore } from "@/modules/camera/store.ts";

import { cn } from "@/shared/lib/utils/cn.ts";
import { CircularButton } from "@/shared/ui/theme/cozy/circular_button.tsx";

/** Toggles whether panning re-samples terrain elevation under the map center. */
export function GroundSnapToggle() {
  const ground_snap = useCameraStore((state) => state.ground_snap);
  const toggle_ground_snap = useCameraStore.getState().toggle_ground_snap;

  return (
    <CircularButton className="w-11 h-11" onClick={toggle_ground_snap} isActive={ground_snap}>
      <Mountain
        className={cn(
          "w-5 h-5 transition-all duration-500 ease-out",
          ground_snap ? "text-amber-500" : "text-[#5B4636]/40",
        )}
      />
    </CircularButton>
  );
}
