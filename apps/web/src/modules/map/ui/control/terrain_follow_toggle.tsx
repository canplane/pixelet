// @pxl/web/modules/map/ui/control/terrain_follow_toggle.tsx

import { Mountain } from "lucide-react";

import { useCameraStore } from "@/modules/camera/store.ts";

import { cn } from "@/shared/lib/utils/cn.ts";
import { CircularButton } from "@/shared/ui/theme/cozy/circular_button.tsx";

/** Toggles whether panning re-samples terrain elevation under the map center. */
export function TerrainFollowToggle() {
  const follow_terrain = useCameraStore((state) => state.follow_terrain);
  const toggle_follow_terrain = useCameraStore.getState().toggle_follow_terrain;

  return (
    <CircularButton className="w-11 h-11" onClick={toggle_follow_terrain} isActive={follow_terrain}>
      <Mountain
        className={cn(
          "w-5 h-5 transition-all duration-500 ease-out",
          follow_terrain ? "text-amber-500" : "text-[#5B4636]/40",
        )}
      />
    </CircularButton>
  );
}
