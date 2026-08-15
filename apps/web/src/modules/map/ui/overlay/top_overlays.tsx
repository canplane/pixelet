// @pxl/web/modules/map/ui/overlay/top_overlays.tsx

import { cn } from "@/shared/lib/utils/cn.ts";
import { CameraStatus, useCameraStore } from "@/modules/camera/store.ts";
import { useMapStore } from "@/modules/map/store.ts";
import { CurrentAddress } from "./current_address.tsx";
import { SensorStatus } from "./sensor_status.tsx";
import { DummyChatText } from "./dummy_chat_text.tsx";
import { useReverseGeocode } from "./use_reverse_geocode.ts";
import { MapCompass } from "../control/compass.tsx";
import { MapScaleBar } from "../control/scale_bar.tsx";
import { TrackingButton } from "@/modules/map/ui/control/tracking_button.tsx";

/**
 * Everything anchored to the top edge of the map canvas — split out of
 * map_overlays.tsx alongside BottomOverlays.
 *
 * Top-right now stacks the address/sensor status bar above the compass /
 * scale_bar / GPS cluster (moved here from bottom-right) — that cluster
 * used to live at the bottom, but the floating mode bar (bottom-center,
 * bottom_overlays.tsx) is planned to grow into this app's main control
 * surface, and per that direction the bottom edge is meant to stay clear
 * for it rather than sharing space with unrelated controls. The debug
 * toggle (dev_panel.tsx, a separate app-level component) has since moved
 * to bottom-left (see its own comment), freeing top-left for
 * DummyChatText instead.
 */
export function TopOverlays() {
  const camera_status: CameraStatus = useCameraStore((state) => state.camera_status);
  const is_camera_busy: boolean = camera_status !== CameraStatus.IDLE;

  const interaction_style = cn(
    "transition-all duration-500 ease-in-out",
    is_camera_busy ? "pointer-events-none opacity-40 scale-[0.98] blur-[0.5px]" : "opacity-100 scale-100 blur-0",
  );

  // owned here (not inside CurrentAddress) so the debounced Nominatim fetch
  // only fires once, no matter how the status bar's pieces end up
  // composed/reused.
  const map_center = useMapStore((state) => state.map_center);
  const { label: address_label } = useReverseGeocode(map_center);

  return (
    <>
      {/* 1. top-center: location search bar */}
      {
        /* <div className="absolute top-8 left-1/2 -translate-x-1/2 z-100 w-[calc(100%-4rem)] flex justify-center pointer-events-none">
        <MapSearchBar className={cn(interaction_style, "pointer-events-auto")} />
      </div> */
      }

      {/* 2. top-left: dummy chat-text preview (mirrors mode_bar.tsx's message input live) */}
      <DummyChatText />

      {/* 3. top-right: sensor status + current location (dong), compass/scale_bar/GPS below it */}
      <div
        className={cn(
          // gap-3 -> gap-5: the address bar and the control row below it
          // read as too close together at gap-3.
          "absolute top-4 right-4 z-50 flex flex-col items-end gap-5 pointer-events-none",
          interaction_style,
        )}
      >
        <div className="flex flex-col items-end gap-1">
          {
            /* h-6 fixed (not py-driven) so the bar doesn't reflow height when
          CurrentAddress's label fades in/out via AnimatePresence —
          items-center does the vertical centering instead of padding. */
          }
          {
            /* Fixed-width icon at the right end (the anchor closest to the
          screen edge), variable-length text on the left — this pill is
          itself pinned to right-4 and w-fit, so the reverse order (text on
          the right) would put the variable element where the fixed anchor
          closest to the screen edge should be, making the icon visibly
          drift left/right every time the address length changed. */
          }
          <div className="flex items-center gap-2 h-6 bg-black/40 backdrop-blur-sm rounded px-2 w-fit">
            <CurrentAddress label={address_label} />
            <SensorStatus />
          </div>
        </div>

        {
          /* scale bar - GPS
                       compass
        — scale bar aligned to the top row (items-start) next to GPS;
        compass sits under GPS in the same right-hand column, not under the
        scale bar. */
        }
        <div className={cn("flex items-start gap-3", !is_camera_busy && "pointer-events-auto")}>
          <MapScaleBar />

          <div className="flex flex-col items-center gap-3">
            <TrackingButton />
            <MapCompass />
          </div>
        </div>

        {
          /* <div className="w-64 pointer-events-auto">
          <WorldInspector />
        </div> */
        }
        {
          /* <div className="w-64 pointer-events-auto">
          <PoiDetailPanel />
        </div> */
        }
      </div>
    </>
  );
}
