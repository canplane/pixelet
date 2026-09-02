// @pxl/web/modules/map/ui/control/tracking_button.tsx

import { Compass, Loader2, Locate, Navigation } from "lucide-react";
import { useTranslation } from "react-i18next";

import { type DeviceLocation, type DeviceOrientation, DevicePoseStatus, useDeviceLocationStore, useDeviceOrientationStore } from "@/modules/device/pose/store.ts";
import { useDeviceOrientation } from "@/modules/device/pose/use_device_orientation.ts";
import { useMapStore } from "@/modules/map/store.ts";
import { useMapNavigation } from "@/modules/map/navigation/use_map_navigation.ts";
import { useCameraStore } from "@/modules/camera/store.ts";
import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { useFollowMode } from "@/modules/map/navigation/use_follow_mode.ts";
import { ground_or_raw } from "@/modules/world/terrain/elevation.ts";

import { cn } from "@/shared/lib/utils/cn.ts";
import { CircularButton } from "@/shared/ui/shadcn/circular_button.tsx";

import type { f64 } from "@xpute/core/kit/word.ts";

export function TrackingButton() {
  const { t } = useTranslation();
  useFollowMode();

  const location_status: DevicePoseStatus = useDeviceLocationStore((state) => state.status);
  const location: DeviceLocation | null = useDeviceLocationStore((state) => state.location);

  const orientation: DeviceOrientation | null = useDeviceOrientationStore((state) => state.orientation);
  const { request_orientation } = useDeviceOrientation();

  const set_map_center = useMapStore.getState().set_map_center;

  const view_state = useCameraStore((state) => state.view_state);
  const is_follow_mode = useCameraStore((state) => state.is_follow_mode);
  const set_is_follow_mode = useCameraStore.getState().set_is_follow_mode;
  const reset_azimuth = useCameraStore.getState().reset_azimuth;

  const { device_coord, is_off_center } = useMapNavigation();

  const can_show_location_control: boolean = is_off_center && !is_follow_mode;

  const { azimuth } = view_state;

  const is_enabled: boolean = location_status === DevicePoseStatus.READY && !!location;
  const is_loading: boolean = location_status === DevicePoseStatus.LOADING;

  const handle_action = async () => {
    if (!is_enabled || is_loading) return;

    if (can_show_location_control) {
      if (device_coord) {
        // device_coord's Z is the device's raw GPS altitude, not the map's
        // baked terrain height — ground_or_raw applies the same terrain
        // sample do_pan (input.tsx) uses, so this recenter lands exactly
        // where panning there normally would.
        set_map_center(
          useSettingsStore.getState().ground_snap ? ground_or_raw(device_coord) : device_coord,
        );
      }
      set_is_follow_mode(false);
      return;
    }

    if (!is_follow_mode) {
      const success: boolean = await request_orientation();
      if (success) {
        set_is_follow_mode(true);
      } else {
        console.error("[orientation] could not start the sensor.");
      }
    } else {
      set_is_follow_mode(false);
      reset_azimuth();
    }
  };

  const get_icon_style = () => {
    if (is_loading) {
      return { Icon: Loader2, className: "animate-spin opacity-40" };
    }

    if (can_show_location_control) {
      return { Icon: Locate, className: "text-[#5B4636]/60 scale-90" };
    }

    if (is_follow_mode) {
      const heading_deg: f64 = orientation?.heading ?? 0;
      const final_rotation_deg: f64 = heading_deg - (azimuth * 180) / Math.PI;

      return {
        Icon: Compass,
        style: { transform: `rotate(${final_rotation_deg}deg)` },
        className: "text-amber-500 scale-110",
      };
    }

    return {
      Icon: Navigation,
      className: "text-[#5B4636]/80 group-hover:rotate-[45deg]",
    };
  };

  const { Icon, className, style } = get_icon_style();

  const title = can_show_location_control ? t("map.controls.move_to_location") : is_follow_mode ? t("map.controls.exit_compass_mode") : t("map.controls.start_compass_mode");

  return (
    <CircularButton
      className="w-12 h-12"
      shadow={false}
      onClick={handle_action}
      isActive={is_follow_mode}
      disabled={!is_enabled || is_loading}
      title={title}
    >
      <Icon
        className={cn(
          "w-5 h-5 transition-all duration-500 ease-out",
          className,
        )}
        style={style}
      />
    </CircularButton>
  );
}
