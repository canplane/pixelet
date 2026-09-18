// @pxl/web/ui/map/overlay/top_overlays.tsx

import { Search } from "lucide-react";
import { useTranslation } from "react-i18next";

import { cn } from "@/kit/lib/util/cn.ts";
import { useAppContext } from "@/app/provider/context.ts";
import { use_camera_busy_interaction_style } from "@/ui/map/use_camera_busy_interaction_style.ts";
import { useKernel } from "@/kernel/kernel.ts";
import { CurrentAddress } from "./current_address.tsx";
import { useReverseGeocode } from "./use_reverse_geocode.ts";
import { SensorStatus } from "./sensor_status.tsx";
import { DummyChatText } from "./dummy_chat_text.tsx";
import { MapCompass } from "../control/compass.tsx";
import { MapScaleBar } from "../control/scale_bar.tsx";
import { TrackingButton } from "@/ui/map/control/tracking_button.tsx";
import { SettingsTriggerButton } from "@/ui/setting/trigger_button.tsx";

export function TopOverlays() {
  const { is_camera_busy, interaction_style } = use_camera_busy_interaction_style();
  const { ui } = useAppContext();
  const { t } = useTranslation();

  const { label: address_label } = useReverseGeocode(useKernel((s) => s.map.page));

  return (
    <>
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
            {
              /* Opens forward search — the current-address label and "where
              do I want to go" share the same mental slot on a map, so this
              is the one trigger for both rather than a separate
              always-visible search bar. A sibling button, not a wrapper
              around the whole pill: SensorStatus is its own clickable
              button right next to it, and nesting one button inside
              another is invalid HTML. */
            }
            <button
              type="button"
              onClick={ui.openSearchModal}
              aria-label={t("map.search.trigger_aria_label")}
              className="min-w-0 flex items-center gap-1.5 pointer-events-auto cursor-pointer"
            >
              {
                /* Always visible, unlike the address text (CurrentAddress
                renders nothing until a label resolves) — without this the
                whole trigger would be an invisible tap target on first
                load or over unmapped water. w-3.5 h-3.5 to match
                SensorStatus's own icons right next to it in this same
                pill, not an arbitrary size of its own. */
              }
              <Search className="w-3.5 h-3.5 text-[#5B4636]/50 shrink-0" />
              <CurrentAddress label={address_label} />
            </button>
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
      </div>
    </>
  );
}
