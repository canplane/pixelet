// @pxl/web/modules/map/ui/control/scale-bar.tsx

import { useCameraStore } from "@/modules/camera/store.ts";
import { camera_cfg } from "@/modules/camera/config.ts";

import { cn } from "@/shared/lib/utils/cn.ts";
import { MotionContainer } from "@/shared/ui/motion/motion-container.tsx";

const { FOV } = camera_cfg.projection;
const { DEFAULT_DIST } = camera_cfg.view;

const SCALE_BAR_WIDTH = 80;

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
            is_default_zoom ? "text-blue-400/80" : "text-white/40 group-hover:text-white/80",
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
              "h-[0.5px] w-full relative transition-colors",
              is_default_zoom ? "bg-blue-400/30" : "bg-white/20",
            )}
          >
            {/* 기본 줌일 때 은은한 블루 글로우 효과 */}
            {is_default_zoom && <div className="absolute inset-0 bg-linear-to-r from-transparent via-blue-400/40 to-transparent blur-[0.5px]" />}
          </div>

          {/* 왼쪽 눈금 */}
          <div
            className={cn(
              "absolute left-0 h-2 w-[0.5px] transition-colors",
              is_default_zoom ? "bg-blue-400/60" : "bg-white/40 group-hover:bg-white/80",
            )}
          />

          {/* 중앙 눈금 */}
          <div
            className={cn(
              "absolute left-1/2 -translate-x-1/2 w-[2px] h-[2px] rounded-full transition-colors",
              is_default_zoom ? "bg-blue-400/40" : "bg-white/20 group-hover:bg-white/50",
            )}
          />

          {/* 오른쪽 눈금 */}
          <div
            className={cn(
              "absolute right-0 h-2 w-[0.5px] transition-colors",
              is_default_zoom ? "bg-blue-400/60" : "bg-white/40 group-hover:bg-white/80",
            )}
          />
        </div>
      </div>
    </MotionContainer>
  );
}
