// @pxl/web/modules/map/ui/control/compass.tsx

import { useCameraStore } from "@/modules/camera/store.ts";
import { camera_cfg } from "@/modules/camera/config.ts";

import { cn } from "@/shared/lib/utils/cn.ts";
import { MotionContainer } from "@/shared/ui/motion/motion-container.tsx";
import { CircularButton } from "@/shared/ui/theme/glass/circular-button.tsx";

const { DEFAULT_POLAR } = camera_cfg.view;

export function MapCompass() {
  const view_state = useCameraStore((state) => state.view_state);
  const reset_orientation = useCameraStore((state) => state.reset_orientation);
  const { dist, azimuth, polar } = view_state;

  const tilt_thres = Math.max(0.26, 2.5 / (dist + 7.5) + 0.1);
  const azimuth_thres = (Math.max(3.0, 25 / (dist + 2.5))) * (Math.PI / 180);

  const is_rotated: boolean = Math.abs(azimuth) > azimuth_thres;
  const is_tilted = Math.abs(polar - DEFAULT_POLAR) > tilt_thres;

  const is_default_orientation: boolean = !is_rotated && !is_tilted;

  return (
    <MotionContainer show={!is_default_orientation} variant="compass" delay={0.4}>
      <CircularButton onClick={reset_orientation} isActive={is_default_orientation}>
        {/* 🧭 N 표시: 방향과 틸트가 모두 완벽히 리셋되었을 때만 빨간색 활성화 */}
        <span
          className={cn(
            "absolute top-1 text-[9px] font-medium tracking-tighter select-none z-20 transition-colors duration-300",
            is_default_orientation // 🚀 추출한 변수를 사용하여 조건문 단순화
              ? "text-red-500 shadow-[0_0_8px_rgba(239,68,68,0.4)]"
              : "text-white/40",
          )}
        >
          N
        </span>

        {/* 🧭 바늘 회전 */}
        <div
          className="relative w-[2px] h-8 flex flex-col z-10 transition-transform duration-200 ease-[cubic-bezier(0.1,0,0,1)]"
          style={{ transform: `rotate(${(-azimuth * 180) / Math.PI}deg)` }}
        >
          <div className="flex-1 bg-red-500/80 rounded-t-full shadow-[0_0_12px_rgba(239,68,68,0.4)]" />
          <div className="flex-1 bg-white/20 rounded-b-full shadow-[0_0_8px_rgba(255,255,255,0.1)]" />
        </div>
      </CircularButton>
    </MotionContainer>
  );
}
