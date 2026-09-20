// @pxl/web/feature/map/ui/control/compass.tsx

import { useTranslation } from "react-i18next";

import { kernel_action, useKernel } from "@/kernel/shell.ts";

import { MotionContainer } from "@/shared/ui/motion/motion-container.tsx";
import { CircularButton } from "@/shared/ui/shadcn/circular-button.tsx";

export function MapCompass() {
  const { t } = useTranslation();
  const azimuth = useKernel((s) => s.view.azimuth);
  const showCompass = useKernel((s) => s.compass.show);

  return (
    // layout={false}: without it, framer-motion's default layout={true}
    // auto-animates this button's bounding box whenever the shared flex
    // column it sits in (top-overlays.tsx) resizes for any reason, which
    // can leave it stuck mid-transition instead of cleanly disappearing.
    <MotionContainer show={showCompass} variant="compass" delay={0.4} layout={false}>
      <CircularButton className="w-12 h-12" shadow={false} onClick={() => kernel_action({ type: "compass_reset" })} title={t("map.controls.reset_orientation")}>
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
