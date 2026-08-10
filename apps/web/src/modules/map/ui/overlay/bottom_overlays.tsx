// @pxl/web/modules/map/ui/overlay/bottom_overlays.tsx

import { useState } from "react";

import { cn } from "@/shared/lib/utils/cn.ts";
import { useDebugStore } from "@/app/dev/debug_store.ts";
import { CameraStatus, useCameraStore } from "@/modules/camera/store.ts";
import { ModeBar } from "./mode_bar.tsx";

/**
 * Everything anchored to the bottom edge of the map canvas — split out of
 * map_overlays.tsx. The compass/scale_bar/GPS cluster that used to live
 * here moved to top_overlays.tsx (top-right) so the bottom edge stays
 * clear for the floating mode bar, which is planned to grow into this
 * app's main control surface (search, text mode, ...) — see
 * top_overlays.tsx's doc comment and overlay_safe_area.ts for the
 * reasoning/shared footprint constants that decision feeds into (the toast
 * container in toaster.tsx still reads BOTTOM_OVERLAY_SAFE_PX to clear the
 * mode bar).
 */
export function BottomOverlays() {
  const camera_status: CameraStatus = useCameraStore((state) => state.camera_status);
  const is_camera_busy: boolean = camera_status !== CameraStatus.IDLE;

  // Still a concept (see this component's own doc comment) — not wired to
  // a real chat/drawing system yet, so it only shows up behind the same
  // debug-menu gate as the rest of the dev-only UI for now. show_debug_menu,
  // not show_debug_objects — the latter is specifically the 3D-scene gizmo
  // toggle (frame-origin spheres etc.), a separate concern with its own
  // button in dev_panel.tsx now that the two used to be conflated.
  const show_debug_menu = useDebugStore((state) => state.show_debug_menu);

  // Both required data-source notices (see this file's attribution block
  // below) collapse behind a tap instead of sitting permanently on-screen —
  // Copernicus's notice alone is too long for a phone-width viewport, and
  // OSM's stays collapsed too rather than getting a different rule just
  // because it happens to be short. Allowed by the OSMF Attribution
  // Guidelines' own accommodation for this ("may be collapsed... [if]
  // discoverable through an information button"). This is a stopgap, not
  // the final home for this content — once a real Settings/About screen
  // exists, this belongs there instead of behind a map-corner ⓘ.
  const [show_data_credits, set_show_data_credits] = useState(false);

  const interaction_style = cn(
    "transition-all duration-500 ease-in-out",
    is_camera_busy ? "pointer-events-none opacity-40 scale-[0.98] blur-[0.5px]" : "opacity-100 scale-100 blur-0",
  );

  return (
    <>
      {/* 4. bottom-left: origin-jump debug utility */}
      <div className={cn("absolute bottom-8 left-8 z-50", interaction_style)} />

      {
        /* Click-outside-to-close backdrop for the data-credits panel below
      — rendered OUTSIDE the interaction_style-bearing wrapper on purpose:
      that wrapper always carries a `scale-*` class (even scale-100 at
      rest), and ANY transform value (identity included) makes an element
      a containing block for its own `position: fixed` descendants — a
      fixed backdrop nested inside it would resolve `inset-0` against that
      small wrapper's own box instead of the real viewport, silently
      failing to catch outside clicks. No explicit z-index needed: it has
      none, so the z-50 panel/button below it win by explicit z-index
      regardless of DOM order. */
      }
      {show_data_credits && (
        <div
          className="fixed inset-0 pointer-events-auto"
          onClick={() => set_show_data_credits(false)}
          aria-hidden="true"
        />
      )}

      {
        /* Data source attribution — the ⓘ trigger itself stays visible
      whenever the map is up (matches the top-4 right-4 top-right address
      menu's own margin for symmetry), independent of whether the
      Nominatim reverse-geocode lookup succeeds, since what it discloses
      covers the whole terrain/road/building bake, not just that lookup.
      Two required notices live behind it: OSM covers roads/buildings/
      coastline + Nominatim, Copernicus WorldDEM-30 covers elevation
      (infra/geo/esa-elevation/) — see infra/geo/README.md's own "Data
      Licensing & Attribution" section for why each notice is worded the
      way it is; neither text is something to edit/shorten. */
      }
      <div
        className={cn(
          "absolute bottom-4 right-4 z-50 text-[8px] text-white/30 text-right leading-tight",
          interaction_style,
        )}
      >
        {
          /* Both notices collapse behind the same disclosure, symmetrically
          — OSM's own text would fit permanently on-screen on its own, but
          leaving it uncollapsed while Copernicus collapses would make the
          two look like they're held to different rules for no real reason.
          The ⓘ and the panel are mutually exclusive (one or the other, never
          both) rather than the ⓘ persisting alongside an expanding panel —
          closing only ever happens via the outside-click backdrop above, so
          there's no separate "close" affordance to keep the ⓘ around for.
          Both share this exact same bottom-4/right-4 anchor, so their outer
          margins are identical by construction, matching the top-4/right-4
          top-right address menu. */
        }
        {show_data_credits
          ? (
            <div className="pointer-events-auto flex w-[min(320px,calc(100vw-2rem))] flex-col gap-2 rounded bg-black/40 backdrop-blur-sm p-2 text-white/70">
              <span>© OpenStreetMap contributors</span>
              <span>
                produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved
              </span>
            </div>
          )
          : (
            <button
              type="button"
              onClick={() => set_show_data_credits(true)}
              aria-label="데이터 출처 표시"
              className="pointer-events-auto flex h-4 w-4 items-center justify-center rounded-full border border-white/30 text-[8px] leading-none hover:text-white/50"
            >
              i
            </button>
          )}
      </div>

      {
        /* 5. bottom-center: floating mode bar (concept). z-40 — the toast
      pill uses its own portal at a much higher z-index, so it floats above
      this; toaster.tsx keeps its own margin via BOTTOM_OVERLAY_SAFE_PX so
      the two don't overlap. Keep this bottom value in sync with
      MODE_BAR_BOTTOM_OFFSET_PX (overlay_safe_area.ts). */
      }
      {show_debug_menu && (
        <div
          // left-6 right-6 (not left-1/2 -translate-x-1/2 + max-w), so the
          // reserved 24px inset — the same number as this bar's own
          // bottom-6 — comes from real layout width, not a clipped/
          // scrollable overflow area: the box's width IS 100vw - 48px,
          // full stop, so there's nothing for content to overflow past in
          // the first place.
          className={cn("absolute bottom-6 left-6 right-6 z-40 pointer-events-none", interaction_style)}
        >
          {
            // w-full + flex justify-center here (not on the div above) —
            // ModeBar's own w-full/flex-1 chain (mode_bar.tsx) needs a
            // real ancestor width to resolve against, not just a centered
            // flex item with no forced size: without this, widening the
            // message input's max-w did nothing visible, because
            // flex-grow was claiming "100% of an undefined width," i.e.
            // nothing. justify-center keeps ModeBar visually centered in
            // the reserved box whenever it doesn't need the full width;
            // when it does, its own min-w-0 chain lets the message pill
            // shrink to fit instead of pushing past the margins.
          }
          <div className={cn("min-w-0 w-full flex justify-center", !is_camera_busy && "pointer-events-auto")}>
            <ModeBar />
          </div>
        </div>
      )}
    </>
  );
}
