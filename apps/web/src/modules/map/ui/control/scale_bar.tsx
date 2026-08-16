// @pxl/web/modules/map/ui/control/scale_bar.tsx

import { useTranslation } from "react-i18next";

import { useCameraStore } from "@/modules/camera/store.ts";
import { camera_cfg } from "@/modules/camera/config.ts";
import { useMapStore } from "@/modules/map/store.ts";

import * as epsg3857 from "@pxl/shared/spec/space/adapter/3857.ts";
import type { G3857 } from "@pxl/shared/spec/space/adapter/3857.ts";

import type { f64 } from "@xpute/core/kit/word.ts";

import { cn } from "@/shared/lib/utils/cn.ts";
import { MotionContainer } from "@/shared/ui/motion/motion_container.tsx";

const { FOV } = camera_cfg.projection;
const { DEFAULT_DIST } = camera_cfg.view;

// layout budget, not the displayed measurement — the bar never renders at
// this width. round_to_nice_scale() picks a "nice" round distance whose
// pixel length is at most this, then the bar is drawn at exactly that
// distance's real width, so the number and the line under it always agree
// to the pixel (instead of a fixed-width bar labeled with whatever
// arbitrary-precision distance fell out of the math, e.g. "37.2m")
const SCALE_BAR_MAX_WIDTH_PX = 64;

// 1/2/5 × 10^n is the standard cartographic scale_bar step sequence (same
// one Leaflet/Mapbox/Google Maps use) — each step is at most a ×2.5 jump
// from the last, small enough that the bar rarely looks empty right after
// it shrinks, and every value is trivial to read and to eyeball-multiply.
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
  // deliberately independent of polar (tilt). Dividing by cos(polar) is
  // geometrically correct for the exact ground distance at the crosshair,
  // but swings the number unboundedly as polar nears the horizon, purely
  // from tilting — reported live as confusing ("why did the number change,
  // I only rotated, not zoomed?"). This bar's job is closer to "how zoomed
  // in am I" than pixel-exact ground meters at one point (not even
  // well-defined for a tilted view — real ground-m/px varies continuously
  // across the screen). 2*tan(fov/2)*dist is the frustum's cross-sectional
  // size at `dist`, independent of polar — a reasonable zoom-level stand-in.
  const visible_u = 2 * Math.tan(fov_rad / 2) * dist;
  // `dist` (like every camera/scene distance here) is in canonical "u"
  // units, not meters — a prior version of this comment wrongly assumed
  // scene distance was already in EPSG:3857 planar meters and skipped this
  // conversion, making every shown distance 1/U_PER_M too small.
  // that bug survived an "empirical" check that only compared two
  // Vector3.distanceTo() results against each other (both already in u
  // units) — caught for real by cross-checking against HoverCursor's
  // on-screen pixel width (scale={U_PER_CELL}, a known true size).
  const visible_planar_m = visible_u / epsg3857.U_PER_M;
  const planar_m_per_px = visible_planar_m / screen_height;
  // the planar-meter plane is 1:1 with real ground meters only at the
  // equator — away from it, a planar meter covers less real ground by
  // cos(lat). Z_SCALE(y) already encodes that as 1/cos(lat) (same factor
  // that keeps vertical extrusion consistent with the horizontal plane),
  // so dividing by it converts back to the real meters this label shows.
  const mercator_scale = map_center !== null ? epsg3857.Z_SCALE(epsg3857.unproject(G3857_SCRATCH, map_center.i, map_center.f)[1]) : 1;
  const real_m_per_px = planar_m_per_px / mercator_scale;

  // nice-round-number scale bar: pick the largest nice distance that fits
  // the pixel budget, then size the bar to exactly that distance. Every
  // value round_to_nice_scale returns has at most one significant digit
  // (1, 2, 5, 10, 20, 50, ...), so no separate rounding policy is needed.
  const total_meters: f64 = round_to_nice_scale(SCALE_BAR_MAX_WIDTH_PX * real_m_per_px);
  const bar_width_px: f64 = real_m_per_px > 0 ? total_meters / real_m_per_px : 0;

  // is the current zoom at its default value (1m tolerance)
  const is_default_zoom: boolean = Math.abs(dist - DEFAULT_DIST) < 1;

  // total_meters is already a "nice" 1/2/5 × 10^n value, so plain string
  // conversion never needs more than the digits the number actually has —
  // no toFixed cascade to keep in sync across magnitude boundaries.
  const label: string = total_meters >= 1000 ? `${total_meters / 1000}km` : `${total_meters}m`;

  return (
    // layout={false}: MotionContainer's motion.div defaults to layout={true},
    // which fakes any bounding-box size change (this bar's width recalculates
    // every zoom tick) via a transform: scale() that visibly stretches the
    // text glyphs until it settles. Disabling it and animating `width`
    // directly below (a real box-model transition) keeps glyphs undistorted.
    <MotionContainer show={!is_default_zoom} variant="riseUp" delay={0.2} layout={false}>
      <div
        onClick={reset_dist}
        // py-2 (top and bottom): restored the top half after a prior pass
        // snugged it to the row's own top edge, which read as too tight
        // next to the GPS button's breathing room.
        //
        // min-w-[44px]: the click/tap target, decoupled from the bar's own
        // visual width (bar_width_px, set on the inner div below) — at some
        // zoom levels that visual width shrinks well under 44px (the usual
        // minimum comfortable touch-target size), which made "reset zoom"
        // hard to land precisely on mobile. items-center on this flex-col
        // keeps the bar/label centered within whatever extra invisible
        // width this adds, so the visible bar itself never changes size or
        // position, only the hit-area around it grows.
        //
        // the outline lives here, on the parent, as ONE filter, not a real
        // CSS `outline` per shape — a real per-element outline (tried
        // first) read as four disconnected halos instead of one cohesive
        // outline, and doesn't follow text glyphs anyway (only the
        // rectangular bounding box). filter: drop-shadow wraps the union of
        // every child's actual rendered shape into one outline instead.
        className="group flex flex-col items-center cursor-pointer transition-all duration-300 active:scale-95 py-2 min-w-[44px] filter-[drop-shadow(1px_0_0_rgba(0,0,0,0.9))_drop-shadow(-1px_0_0_rgba(0,0,0,0.9))_drop-shadow(0_1px_0_rgba(0,0,0,0.9))_drop-shadow(0_-1px_0_rgba(0,0,0,0.9))]"
        title={t("map.scale_bar.reset_zoom")}
      >
        {/* 1. scale bar — width matches exactly the distance the label shows. */}
        <div
          // transition-[width]: a genuine box-model width tween (not a
          // transform), so the bar visibly grows/shrinks in place without
          // scaling anything inside it — this is the "just animate width"
          // half of the fix; layout={false} above is the other half.
          className="relative flex items-center transition-[width] duration-300 ease-out"
          style={{ width: `${bar_width_px}px` }}
        >
          {/* base horizontal line */}
          <div
            className={cn(
              "h-px w-full relative transition-colors",
              is_default_zoom ? "bg-amber-400" : "bg-white",
            )}
          />

          {/* left tick */}
          <div
            className={cn(
              "absolute left-0 h-2 w-px transition-colors",
              is_default_zoom ? "bg-amber-400" : "bg-white",
            )}
          />

          {/* right tick */}
          <div
            className={cn(
              "absolute right-0 h-2 w-px transition-colors",
              is_default_zoom ? "bg-amber-400" : "bg-white",
            )}
          />
        </div>

        {
          /* 2. scale label — accent color at the default zoom. The filter
          above wraps this text into the same single outline too. */
        }
        <span
          className={cn(
            // SI units are case-sensitive: m (meter) != M (mega), km != KM —
            // the `uppercase` that used to be here was force-uppercasing a
            // label string that already composed its unit lowercase correctly.
            "text-[8px] font-bold mt-1 tracking-widest transition-colors",
            is_default_zoom ? "text-amber-400" : "text-white",
          )}
        >
          {label}
        </span>
      </div>
    </MotionContainer>
  );
}
