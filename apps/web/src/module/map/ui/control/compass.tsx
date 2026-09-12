// @pxl/web/module/map/ui/control/compass.tsx

import { useTranslation } from "react-i18next";

import { useCameraStore } from "@/module/camera/store.ts";

import { MotionContainer } from "@/shared/ui/motion/motion_container.tsx";
import { CircularButton } from "@/shared/ui/shadcn/circular_button.tsx";

import { reground_map_center } from "@/module/world/terrain/use_ground_snap_sync.ts";

export function MapCompass() {
  const { t } = useTranslation();
  const view_state = useCameraStore((state) => state.view_state);
  const is_follow_mode = useCameraStore((state) => state.is_follow_mode);
  const reset_azimuth = useCameraStore.getState().reset_azimuth;
  const { dist, azimuth } = view_state;

  const azimuth_thres = (Math.max(3.0, 25 / (dist + 2.5))) * (Math.PI / 180);

  const is_rotated: boolean = Math.abs(azimuth) > azimuth_thres;

  const is_default_orientation: boolean = !is_rotated;

  const show_compass: boolean = !is_default_orientation && !is_follow_mode;

  // reset_azimuth doesn't re-sample elevation, so map_center can be left
  // stale relative to the new angle if it isn't re-grounded alongside it.
  const handle_reset = () => {
    reset_azimuth();
    reground_map_center();
  };

  return (
    // layout={false}: without it, framer-motion's default layout={true}
    // auto-animates this button's bounding box whenever the shared flex
    // column it sits in (top_overlays.tsx) resizes for any reason, which
    // can leave it stuck mid-transition instead of cleanly disappearing.
    <MotionContainer show={show_compass} variant="compass" delay={0.4} layout={false}>
      <CircularButton className="w-12 h-12" shadow={false} onClick={handle_reset} title={t("map.controls.reset_orientation")}>
        <span className="absolute top-0.5 text-[10px] font-medium tracking-tighter select-none z-20 text-[#5B4636]/50">
          N
        </span>

        <div
          className="relative w-[2px] h-6 flex flex-col z-10 transition-transform duration-200 ease-[cubic-bezier(0.1,0,0,1)]"
          style={{ transform: `rotate(${(-azimuth * 180) / Math.PI}deg)` }}
        >
          <div className="flex-1 bg-red-500/80 rounded-t-full shadow-[0_0_12px_rgba(239,68,68,0.4)]" />
          <div className="flex-1 bg-[#5B4636]/30 rounded-b-full" />
        </div>
      </CircularButton>
    </MotionContainer>
  );
}
