// @pxl/web/modules/map/ui/control/compass.tsx

import { useCameraStore } from "@/modules/camera/store.ts";
import { camera_cfg } from "@/modules/camera/config.ts";

import { cn } from "@/shared/lib/utils/cn.ts";
import { MotionContainer } from "@/shared/ui/motion/motion-container.tsx";
import { CircularButton } from "@/shared/ui/theme/cozy/circular-button.tsx";

import { reground_map_center } from "@/modules/world/terrain/use-terrain-follow-sync.ts";

const { DEFAULT_POLAR } = camera_cfg.view;

export function MapCompass() {
  const view_state = useCameraStore((state) => state.view_state);
  const reset_orientation = useCameraStore.getState().reset_orientation;
  const { dist, azimuth, polar } = view_state;

  const tilt_thres = Math.max(0.26, 2.5 / (dist + 7.5) + 0.1);
  const azimuth_thres = (Math.max(3.0, 25 / (dist + 2.5))) * (Math.PI / 180);

  const is_rotated: boolean = Math.abs(azimuth) > azimuth_thres;
  const is_tilted = Math.abs(polar - DEFAULT_POLAR) > tilt_thres;

  const is_default_orientation: boolean = !is_rotated && !is_tilted;

  // reset_orientation only touches azimuth/polar — it doesn't re-sample elevation, so
  // if map_center had drifted (or was never grounded) the view can visibly "float" once
  // the angle change makes that mismatch obvious. Re-ground alongside the angle reset,
  // same as every other camera-settling interaction (see reground_map_center's callers).
  const handle_reset = () => {
    reset_orientation();
    reground_map_center();
  };

  return (
    <MotionContainer show={!is_default_orientation} variant="compass" delay={0.4}>
      <CircularButton className="w-11 h-11" onClick={handle_reset} isActive={is_default_orientation}>
        {/* 🧭 N 표시: 방향과 틸트가 모두 완벽히 리셋되었을 때만 빨간색 활성화 */}
        <span
          className={cn(
            "absolute top-0.5 text-[8px] font-medium tracking-tighter select-none z-20 transition-colors duration-300",
            is_default_orientation // 🚀 추출한 변수를 사용하여 조건문 단순화
              ? "text-red-500 shadow-[0_0_8px_rgba(239,68,68,0.4)]"
              : "text-[#5B4636]/40",
          )}
        >
          N
        </span>

        {/* 🧭 바늘 회전 */}
        <div
          className="relative w-[2px] h-6 flex flex-col z-10 transition-transform duration-200 ease-[cubic-bezier(0.1,0,0,1)]"
          style={{ transform: `rotate(${(-azimuth * 180) / Math.PI}deg)` }}
        >
          <div className="flex-1 bg-red-500/80 rounded-t-full shadow-[0_0_12px_rgba(239,68,68,0.4)]" />
          <div className="flex-1 bg-[#5B4636]/20 rounded-b-full" />
        </div>
      </CircularButton>
    </MotionContainer>
  );
}
