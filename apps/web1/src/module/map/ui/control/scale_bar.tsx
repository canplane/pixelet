// @pxl/web/module/map/ui/control/scale_bar.tsx

import { useTranslation } from "react-i18next";

import { useCameraStore } from "@/module/camera/store.ts";
import { camera_cfg } from "@/module/camera/config.ts";
import { useMapStore } from "@/module/map/store.ts";

import * as epsg3857 from "@pxl/shared/spec/space/adapter/3857.ts";
import type { G3857 } from "@pxl/shared/spec/space/adapter/3857.ts";

import type { f64 } from "@xpute/core/kit/word.ts";

import { cn } from "@/shared/lib/util/cn.ts";
import { MotionContainer } from "@/shared/ui/motion/motion_container.tsx";

const { FOV } = camera_cfg.projection;
const { DEFAULT_DIST } = camera_cfg.view;

const SCALE_BAR_MAX_WIDTH_PX = 56;
const SCALE_BAR_TICK_PX = 6;
const SCALE_BAR_PAD_PX = 3;

function round_to_nice_scale(raw: f64): f64 {
  if (!(raw > 0)) return 0;

  const exponent: f64 = Math.floor(Math.log10(raw));
  const base: f64 = 10 ** exponent;
  const fraction: f64 = raw / base;

  const nice_fraction: f64 = fraction < 1.5 ? 1 : fraction < 3.5 ? 2 : fraction < 7.5 ? 5 : 10;
  return nice_fraction * base;
}

const G3857_SCRATCH: G3857 = [0, 0, 0];

export function MapScaleBar() {
  const { t } = useTranslation();
  const view_state = useCameraStore((state) => state.view_state);
  const reset_dist = useCameraStore.getState().reset_dist;
  const map_center = useMapStore((state) => state.map_center);

  const { dist } = view_state;

  const screen_height = globalThis.innerHeight ?? 800;
  const fov_rad = (FOV * Math.PI) / 180;
  const visible_u = 2 * Math.tan(fov_rad / 2) * dist;
  // `dist` is in canonical "u" units, not meters — the conversion below is
  // required. A ratio of two Vector3.distanceTo() results can't catch a
  // missing conversion since both sides would already be in u units.
  const visible_planar_m = visible_u / epsg3857.U_PER_M;
  const planar_m_per_px = visible_planar_m / screen_height;
  const mercator_scale = map_center !== null ? epsg3857.Z_SCALE(epsg3857.unproject(G3857_SCRATCH, map_center.i, map_center.f)[1]) : 1;
  const real_m_per_px = planar_m_per_px / mercator_scale;

  const total_meters: f64 = round_to_nice_scale(SCALE_BAR_MAX_WIDTH_PX * real_m_per_px);
  const bar_width_px: f64 = real_m_per_px > 0 ? total_meters / real_m_per_px : 0;

  const is_default_zoom: boolean = Math.abs(dist - DEFAULT_DIST) < 1;

  const label: string = total_meters >= 1000 ? `${total_meters / 1000}km` : `${total_meters}m`;

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
