// @pxl/web/modules/map/ui/control/compass.tsx

import { useTranslation } from "react-i18next";

import { useCameraStore } from "@/modules/camera/store.ts";

import { MotionContainer } from "@/shared/ui/motion/motion_container.tsx";
import { CircularButton } from "@/shared/ui/theme/cozy/circular_button.tsx";

import { reground_map_center } from "@/modules/world/terrain/use_ground_snap_sync.ts";

export function MapCompass() {
  const { t } = useTranslation();
  const view_state = useCameraStore((state) => state.view_state);
  const reset_azimuth = useCameraStore.getState().reset_azimuth;
  const { dist, azimuth } = view_state;

  const azimuth_thres = (Math.max(3.0, 25 / (dist + 2.5))) * (Math.PI / 180);

  const is_rotated: boolean = Math.abs(azimuth) > azimuth_thres;

  // visibility is azimuth-only now — this button's own action
  // (reset_azimuth) only ever touches azimuth, not tilt (see handle_reset's
  // comment), so showing/hiding it for tilt changes too was inconsistent:
  // it could appear while already pointing exactly north (tilt merely
  // off-default), where clicking it would visibly do nothing, or disappear
  // while still rotated because tilt happened to be back at default.
  const is_default_orientation: boolean = !is_rotated;

  // reset_azimuth only touches azimuth — it doesn't re-sample elevation, so if
  // map_center had drifted (or was never grounded) the view can visibly "float" once
  // the angle change makes that mismatch obvious. Re-ground alongside the angle reset,
  // same as every other camera-settling interaction (see reground_map_center's callers).
  const handle_reset = () => {
    reset_azimuth();
    reground_map_center();
  };

  return (
    // layout={false}: this now sits in a flex column alongside
    // TrackingButton (top_overlays.tsx puts GPS above compass in one
    // shared column) instead of its own standalone wrapper — without this,
    // framer-motion's default layout={true} tries to auto-animate this
    // button's bounding box whenever the *column's* size changes (e.g. its
    // own mount/unmount reflowing the shared flex container), which could
    // leave it stuck mid-transition instead of cleanly disappearing.
    <MotionContainer show={!is_default_orientation} variant="compass" delay={0.4} layout={false}>
      {
        /* isActive dropped: MotionContainer only mounts this while
          !is_default_orientation, so is_default_orientation is always false
          for every render that actually reaches the screen — passing it as
          isActive was dead code, CircularButton's amber ring could never
          actually show. This is a one-shot reset button, not a toggle with
          a real "currently active" state to indicate. */
      }
      <CircularButton className="w-11 h-11" dark shadow={false} onClick={handle_reset} title={t("map.controls.reset_orientation")}>
        {
          /* N label — same dead-branch reasoning as the dropped isActive prop
        above: MotionContainer only mounts this while !is_default_orientation,
        so is_default_orientation is always false here. The "red when
        pointing exactly north" branch could never actually render. */
        }
        <span className="absolute top-0.5 text-[8px] font-medium tracking-tighter select-none z-20 text-white/40">
          N
        </span>

        {/* needle rotation */}
        <div
          className="relative w-[2px] h-6 flex flex-col z-10 transition-transform duration-200 ease-[cubic-bezier(0.1,0,0,1)]"
          style={{ transform: `rotate(${(-azimuth * 180) / Math.PI}deg)` }}
        >
          <div className="flex-1 bg-red-500/80 rounded-t-full shadow-[0_0_12px_rgba(239,68,68,0.4)]" />
          <div className="flex-1 bg-white/25 rounded-b-full" />
        </div>
      </CircularButton>
    </MotionContainer>
  );
}
