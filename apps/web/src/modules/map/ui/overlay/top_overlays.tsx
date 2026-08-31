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
import { SettingsTriggerButton } from "@/app/settings/settings_trigger_button.tsx";

export function TopOverlays() {
  const camera_status: CameraStatus = useCameraStore((state) => state.camera_status);
  const is_camera_busy: boolean = camera_status !== CameraStatus.IDLE;

  const interaction_style = cn(
    "transition-[opacity,transform,filter] duration-500 ease-in-out",
    is_camera_busy ? "pointer-events-none opacity-40 scale-[0.98] blur-[0.5px]" : "opacity-100 scale-100 blur-0",
  );

  const map_center = useMapStore((state) => state.map_center);
  const { label: address_label } = useReverseGeocode(map_center);

  return (
    <>
      {
        /* <div className="absolute top-8 left-1/2 -translate-x-1/2 z-100 w-[calc(100%-4rem)] flex justify-center pointer-events-none">
        <MapSearchBar className={cn(interaction_style, "pointer-events-auto")} />
      </div> */
      }

      <div className={cn("absolute top-[calc(1rem+var(--vv-top,0px))] left-4 z-50 flex flex-col items-start gap-3", interaction_style)}>
        <div className="pointer-events-auto">
          <SettingsTriggerButton />
        </div>
        {__IS_DEV__ && <DummyChatText />}
      </div>

      <div
        className={cn(
          "absolute top-[calc(1rem+var(--vv-top,0px))] right-4 z-50 flex flex-col items-end gap-4 pointer-events-none",
          interaction_style,
        )}
      >
        <div className="flex flex-col items-end gap-1">
          {
            /* Solid, not translucent — same as mode_bar.tsx's own chrome
          (its tool strip is a flat bg-neutral-900, not an alpha'd one), and
          matches the round controls' own solid cream bg-[#FFFDF7]. */
          }
          <div className="flex items-center gap-2 h-6 bg-[#FFFDF7] rounded-full px-3 w-fit">
            <CurrentAddress label={address_label} />
            <SensorStatus />
          </div>
        </div>

        <div className={cn("flex items-start gap-2.5", !is_camera_busy && "pointer-events-auto")}>
          <MapScaleBar />

          <div className="flex flex-col items-center gap-2.5">
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
