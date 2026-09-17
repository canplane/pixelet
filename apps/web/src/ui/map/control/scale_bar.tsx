// @pxl/web/ui/map/control/scale_bar.tsx

import { useTranslation } from "react-i18next";

import { kernel_action, useKernel } from "@/kernel/kernel.ts";

import { cn } from "@/kit/lib/util/cn.ts";
import { MotionContainer } from "@/kit/ui/motion/motion_container.tsx";

const SCALE_BAR_TICK_PX = 6;
const SCALE_BAR_PAD_PX = 3;

export function MapScaleBar() {
  const { t } = useTranslation();
  const { total_meters, bar_width_px, is_default_zoom } = useKernel((s) => s.scale_bar);
  const label: string = total_meters >= 1000 ? `${total_meters / 1000}km` : `${total_meters}m`;
  const reset_dist = () => kernel_action({ type: "reset_dist" });

  return (
    // layout={false}: MotionContainer's motion.div defaults to layout={true},
    // which fakes a bounding-box size change via a transform: scale() —
    // visibly stretches this bar's text glyphs on every zoom tick. Animating
    // `width` directly below instead keeps glyphs undistorted.
    <MotionContainer show={!is_default_zoom} variant="riseUp" delay={0.2} layout={false}>
      <div
        onClick={reset_dist}
        // box-shadow/text-shadow, not filter: drop-shadow — a filter
        // interacts badly with transforms and opacity on WebKit (clips a
        // transformed child, vanishes during an opacity fade) in a way
        // Chromium does not.
        className="group flex flex-col items-center cursor-pointer transition-all duration-300 active:scale-95 pt-1 pb-2 min-w-[44px]"
        title={t("map.scale_bar.reset_zoom")}
      >
        <svg
          width={bar_width_px + SCALE_BAR_PAD_PX * 2}
          height={SCALE_BAR_TICK_PX + SCALE_BAR_PAD_PX * 2}
          viewBox={`0 0 ${bar_width_px + SCALE_BAR_PAD_PX * 2} ${SCALE_BAR_TICK_PX + SCALE_BAR_PAD_PX * 2}`}
          aria-hidden="true"
        >
          {(() => {
            const left = SCALE_BAR_PAD_PX + 0.5;
            const right = SCALE_BAR_PAD_PX + bar_width_px - 0.5;
            const top = SCALE_BAR_PAD_PX;
            const bottom = SCALE_BAR_PAD_PX + SCALE_BAR_TICK_PX;
            const mid = top + SCALE_BAR_TICK_PX / 2;
            const d = `M${left} ${top}V${bottom} M${left} ${mid}H${right} M${right} ${top}V${bottom}`;
            return (
              <>
                <path d={d} stroke="rgba(0,0,0,0.9)" strokeWidth={3.5} fill="none" strokeLinecap="square" />
                <path
                  d={d}
                  className={cn("transition-colors", is_default_zoom ? "stroke-amber-400" : "stroke-white")}
                  strokeWidth={1.5}
                  fill="none"
                  strokeLinecap="square"
                />
              </>
            );
          })()}
        </svg>

        <span
          className={cn(
            // SI units are case-sensitive: m (meter) != M (mega), km != KM —
            // the label string must not be force-uppercased.
            "text-[8px] font-bold -mt-0.5 tracking-widest [text-shadow:1px_0_0_rgba(0,0,0,0.9),-1px_0_0_rgba(0,0,0,0.9),0_1px_0_rgba(0,0,0,0.9),0_-1px_0_rgba(0,0,0,0.9),1px_1px_0_rgba(0,0,0,0.9),-1px_1px_0_rgba(0,0,0,0.9),1px_-1px_0_rgba(0,0,0,0.9),-1px_-1px_0_rgba(0,0,0,0.9)] transition-colors",
            is_default_zoom ? "text-amber-400" : "text-white",
          )}
        >
          {label}
        </span>
      </div>
    </MotionContainer>
  );
}
