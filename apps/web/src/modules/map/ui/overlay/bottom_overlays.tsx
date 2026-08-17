// @pxl/web/modules/map/ui/overlay/bottom_overlays.tsx

import { cn } from "@/shared/lib/utils/cn.ts";
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
 *
 * The required OSM/Copernicus data-source attribution used to collapse
 * behind a bottom-right ⓘ here — moved to the settings modal's own license
 * section (app/settings/settings_modal.tsx), reachable from the always-
 * visible top-left settings trigger, per the OSMF Attribution Guidelines'
 * accommodation for collapsing it behind a discoverable information control.
 */
export function BottomOverlays() {
  const camera_status: CameraStatus = useCameraStore((state) => state.camera_status);
  const is_camera_busy: boolean = camera_status !== CameraStatus.IDLE;

  const interaction_style = cn(
    // Not transition-all: these containers position themselves from
    // --vv-top/--vv-bottom, and `all` swept top/bottom into the same 500ms
    // ease. The result was an overlay easing toward where the keyboard had
    // been half a second ago — a motion of its own, visibly not attached to
    // anything. Only the properties this state actually changes are listed,
    // so viewport tracking is immediate and the fade still eases.
    "transition-[opacity,transform,filter] duration-500 ease-in-out",
    is_camera_busy ? "pointer-events-none opacity-40 scale-[0.98] blur-[0.5px]" : "opacity-100 scale-100 blur-0",
  );

  return (
    <>
      {/* 4. bottom-left: origin-jump debug utility */}
      <div className={cn("absolute bottom-8 left-8 z-50", interaction_style)} />

      {
        /* 5. bottom-center: floating mode bar. z-40 — the toast pill uses
      its own portal at a much higher z-index, so it floats above this;
      toaster.tsx keeps its own margin via BOTTOM_OVERLAY_SAFE_PX so the two
      don't overlap. Keep this bottom value in sync with
      MODE_BAR_BOTTOM_OFFSET_PX (overlay_safe_area.ts). No longer gated
      behind the debug menu — its palette pill now drives real object
      placement/erasure (see mode_bar.tsx's own doc comment), not just the
      cosmetic concept it started as. */
      }
      <div
        // left-4 right-4 (not left-1/2 -translate-x-1/2 + max-w), so the
        // reserved 16px inset — the same number as this bar's own
        // bottom-4 — comes from real layout width, not a clipped/
        // scrollable overflow area: the box's width IS 100vw - 32px,
        // full stop, so there's nothing for content to overflow past in
        // the first place.
        // 16 rather than the 24 this used: it is what the top overlays
        // already sit at, and what iOS's own floating keyboard accessory
        // bar uses — measured at 16pt on device, against this bar's 24.
        // Being the one element inset further than everything else is
        // what read as misaligned.
        // --vv-bottom lifts this clear of the on-screen keyboard. The message
        // input lives in here, so without it the bar this row exists to show
        // is the one thing the keyboard covers. Zero when no keyboard is up,
        // which is every other moment.
        // A short ease on `bottom` alone. iOS delivers visualViewport events
        // inconsistently — streaming through the keyboard animation when it
        // opens fresh, and as a single final value when the keyboard was
        // already up — so the bar tracked smoothly sometimes and snapped
        // others. 150ms is short enough to stay attached in the streaming
        // case and long enough to make the snap read as motion rather than a
        // jump. Only `bottom`: the opacity/blur state above still owns its
        // own, slower transition.
        className={cn("absolute bottom-[calc(1rem+var(--vv-bottom,0px))] left-4 right-4 z-40 pointer-events-none transition-[bottom] duration-150 ease-out", interaction_style)}
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
    </>
  );
}
