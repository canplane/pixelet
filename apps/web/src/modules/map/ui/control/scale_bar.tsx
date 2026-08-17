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
// 64 before. This is the one number that decides how much room the bar
// takes horizontally — round_to_nice_scale picks the largest 1/2/5 step
// whose pixel length fits under it — and at 64 the bar was claiming more
// width than a readout nobody reads continuously deserves. Not lower than
// this: a step lands anywhere in [budget/2.5, budget], so the shortest bar
// this can draw is ~22px, and past that the label starts coming out wider
// than the bar it belongs to.
const SCALE_BAR_MAX_WIDTH_PX = 56;

/** Height of the end ticks. Down from 8 with the width, so the |—| keeps
 * its proportions rather than getting stubby. */
const SCALE_BAR_TICK_PX = 6;

/** Room the svg adds on every side so the outline stroke has somewhere to go.
 * A 4-wide stroke reaches 2px past the path it outlines, so a tick drawn at
 * x=0 has half its outline outside the box, where it is clipped. That showed
 * first as the shadow cut off along the top and bottom, and — once only the
 * vertical axis had been padded — as the end ticks having no outline on their
 * outer sides at all. Both axes now. */
const SCALE_BAR_PAD_PX = 3;

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
        // pt-1 pb-2, not py-2 on both: the row this sits in is items-start,
        // so the top padding is the only thing deciding how far below the
        // GPS button's own top edge the bar lands. 8 sat it a touch low
        // against that button; 4 is the small nudge up. The bottom half
        // stays at 8 — it is what separates the label from the compass
        // underneath, which is a different gap with a different job.
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
        // The outline used to be one filter: drop-shadow(...) x4 here on the
        // parent, chosen because a per-element CSS `outline` read as four
        // disconnected halos and does not follow text glyphs. It worked, and
        // on iOS Safari it broke two other things: the bar's left edge was
        // clipped once its children were transformed, and the whole outline
        // vanished while the overlay layer faded its opacity. Chromium showed
        // neither — a filter interacting badly with transforms and opacity is
        // a WebKit trait, not something to work around case by case.
        //
        // Same result without a filter: box-shadow rings the rectangles
        // exactly (they are rectangles, so a ring is the outline), and
        // text-shadow follows the glyphs, which is what drop-shadow was
        // approximating for the label anyway. No filter means no filter
        // region to clip against and nothing for an opacity transition to
        // drop.
        className="group flex flex-col items-center cursor-pointer transition-all duration-300 active:scale-95 pt-1 pb-2 min-w-[44px]"
        title={t("map.scale_bar.reset_zoom")}
      >
        {
          /* 1. scale bar — one SVG path, drawn twice.
             |—| as a single path means one outline instead of three: the
             previous version was three divs, each ringed by its own
             box-shadow, and the rings met at the corners as visible seams on
             every platform. Before that it was a filter: drop-shadow on the
             parent, which produced one cohesive outline and broke on iOS
             Safari — clipping the left edge once children were transformed,
             and disappearing entirely while the overlay layer faded. A path
             stroked wide-and-dark then narrow-and-light is the outline, with
             no filter to go wrong and nothing to seam.

             Drawn at its real width, with no transform and no length
             animation. Three attempts went the other way — width tween, then
             scaleX, then scaleX with vector-effect="non-scaling-stroke" — and
             each broke somewhere: ghosting on iOS, then strokes thinning and
             stretching mid-transition, since non-scaling-stroke is not
             honoured consistently while a transform is being interpolated.

             The length does not need to animate. This bar moves in 1/2/5
             steps, and the cartographic convention it follows (Google Maps,
             Mapbox, Leaflet) snaps between them rather than tweening. Dropping
             the tween removes every one of those failures at once and costs a
             flourish nobody asked for. Colour still transitions — that one is
             a plain property and never misbehaved. */
        }
        <svg
          width={bar_width_px + SCALE_BAR_PAD_PX * 2}
          height={SCALE_BAR_TICK_PX + SCALE_BAR_PAD_PX * 2}
          viewBox={`0 0 ${bar_width_px + SCALE_BAR_PAD_PX * 2} ${SCALE_BAR_TICK_PX + SCALE_BAR_PAD_PX * 2}`}
          aria-hidden="true"
        >
          {/* left tick, mid line, right tick — the .5 offsets put each stroke
              on a pixel centre so a 1px line lands crisp rather than split
              across two rows. */}
          {(() => {
            // Inset on both axes by the padding, so the bar's own length is
            // still exactly bar_width_px while its outline has room outside.
            const left = SCALE_BAR_PAD_PX + 0.5;
            const right = SCALE_BAR_PAD_PX + bar_width_px - 0.5;
            const top = SCALE_BAR_PAD_PX;
            const bottom = SCALE_BAR_PAD_PX + SCALE_BAR_TICK_PX;
            const mid = top + SCALE_BAR_TICK_PX / 2;
            const d = `M${left} ${top}V${bottom} M${left} ${mid}H${right} M${right} ${top}V${bottom}`;
            return (
              <>
                {/* 3.5 against a 1.5 body leaves exactly 1px of dark on each
                    side, which is what the label's own 1px text-shadow draws.
                    They were 1.25 and 1.0 before — close enough to look like
                    a mistake rather than a difference. The body stays at 1.5
                    even now that the bar is shorter and the label is 8px;
                    that is the part that had read as too thin, and shrinking
                    the bar is not a reason to bring the complaint back. */}
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

        {
          /* 2. scale label — accent colour at the default zoom. Outlined by
          its own text-shadow, which follows the glyphs; the svg above carries
          its outline in its own stroke. The column's items-center lines the
          two up, now that the bar is sized to itself rather than drawn inside
          a fixed box. */
        }
        <span
          className={cn(
            // Eight offsets, not four. The cardinal-only version left the
            // diagonals uncovered, so every corner of every glyph thinned to
            // nothing and the label read as more lightly outlined than the
            // bar above it — which carries a full 1.25px on each side from
            // its own stroke pair. The four diagonals close those gaps.
            //
            // -mt-0.5 rather than mt-1: the svg now carries SCALE_BAR_PAD_PX
            // of its own space below the bar, so the old margin sat the label
            // that much further off than it looks in the markup. Pulling back
            // past zero closes the gap the padding opened.
            //
            // SI units are case-sensitive: m (meter) != M (mega), km != KM —
            // the `uppercase` that used to be here was force-uppercasing a
            // label string that already composed its unit lowercase correctly.
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
