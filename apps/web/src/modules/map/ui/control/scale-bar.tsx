// @pxl/web/modules/map/ui/control/scale-bar.tsx

import { useCameraStore } from "@/modules/camera/store.ts";
import { camera_cfg } from "@/modules/camera/config.ts";

import { cn } from "@/shared/lib/utils/cn.ts";
import { MotionContainer } from "@/shared/ui/motion/motion-container.tsx";

const { FOV } = camera_cfg.projection;
const { DEFAULT_DIST } = camera_cfg.view;

const SCALE_BAR_WIDTH = 64;

export function MapScaleBar() {
  const view_state = useCameraStore((state) => state.view_state);
  const reset_dist = useCameraStore.getState().reset_dist;

  const { dist } = view_state;

  const screen_height = typeof window !== "undefined" ? window.innerHeight : 800;
  const fov_rad = (FOV * Math.PI) / 180;
  const visible_meters = 2 * Math.tan(fov_rad / 2) * dist;
  const meters_per_pixel = visible_meters / screen_height;
  const total_meters = SCALE_BAR_WIDTH * meters_per_pixel;

  // 🚀 현재 줌이 기본값인지 체크 (오차범위 1m)
  const is_default_zoom: boolean = Math.abs(dist - DEFAULT_DIST) < 1;

  const label: string = total_meters >= 1000 ? `${(total_meters / 1000).toFixed(total_meters >= 10000 ? 0 : 1)}km` : `${Math.round(total_meters)}m`;

  return (
    <MotionContainer show={!is_default_zoom} variant="drop" delay={0.2}>
      <div
        onClick={reset_dist}
        className="group flex flex-col items-center cursor-pointer transition-all duration-300 active:scale-95 py-2"
        title="줌 리셋"
      >
        {/* 1. 축척 텍스트: 기본 줌일 때 색상 강조 */}
        <span
          className={cn(
            "text-[8px] font-medium mb-2 tracking-widest uppercase transition-colors",
            is_default_zoom ? "text-sky-500/90" : "text-[#5B4636]/40 group-hover:text-[#5B4636]/80",
          )}
        >
          {label}
        </span>

        {/* 2. 축척 막대 */}
        <div
          className="relative flex items-center"
          style={{ width: `${SCALE_BAR_WIDTH}px` }}
        >
          {/* 베이스 수평선 */}
          <div
            className={cn(
              "h-[1px] w-full relative transition-colors",
              is_default_zoom ? "bg-sky-400/50" : "bg-[#5B4636]/20",
            )}
          />

          {/* 왼쪽 눈금 */}
          <div
            className={cn(
              "absolute left-0 h-2 w-[1px] transition-colors",
              is_default_zoom ? "bg-sky-500/70" : "bg-[#5B4636]/40 group-hover:bg-[#5B4636]/80",
            )}
          />

          {/* 중앙 눈금 */}
          <div
            className={cn(
              "absolute left-1/2 -translate-x-1/2 w-[2px] h-[2px] rounded-full transition-colors",
              is_default_zoom ? "bg-sky-400/50" : "bg-[#5B4636]/20 group-hover:bg-[#5B4636]/50",
            )}
          />

          {/* 오른쪽 눈금 */}
          <div
            className={cn(
              "absolute right-0 h-2 w-[1px] transition-colors",
              is_default_zoom ? "bg-sky-500/70" : "bg-[#5B4636]/40 group-hover:bg-[#5B4636]/80",
            )}
          />
        </div>
      </div>
    </MotionContainer>
  );
}
