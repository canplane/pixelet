// @pxl/web/modules/map/ui/control/tracking_button.tsx

import { Compass, Loader2, Locate, Navigation } from "lucide-react";
import { useTranslation } from "react-i18next";

import { type DeviceLocation, type DeviceOrientation, DevicePoseStatus, useDeviceLocationStore, useDeviceOrientationStore } from "@/modules/device/pose/store.ts";
import { useDeviceOrientation } from "@/modules/device/pose/use_device_orientation.ts";
import { useMapStore } from "@/modules/map/store.ts";
import { useMapNavigation } from "@/modules/map/navigation/use_map_navigation.ts";
import { useCameraStore } from "@/modules/camera/store.ts";
import { useFollowMode } from "@/modules/map/navigation/use_follow_mode.ts";
import { ground_or_raw } from "@/modules/world/terrain/elevation.ts";

import { cn } from "@/shared/lib/utils/cn.ts";
import { CircularButton } from "@/shared/ui/theme/cozy/circular_button.tsx";

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

  // show "move to my location" only when off-center and not following.
  const can_show_location_control: boolean = is_off_center && !is_follow_mode;

  const { azimuth } = view_state;

  const is_enabled: boolean = location_status === DevicePoseStatus.READY && !!location;
  const is_loading: boolean = location_status === DevicePoseStatus.LOADING;

  const handle_action = async () => {
    if (!is_enabled || is_loading) return;

    // 1. off-center? -> jump straight back to my location
    if (can_show_location_control) {
      if (device_coord) {
        // device_coord's own Z is whatever the device's raw GPS sensor
        // reported for altitude — not the map's actual baked terrain
        // height. Recentering straight onto it used to visibly disagree
        // with where a normal pan/ground_snap grounds the camera (real
        // content — debug markers, anything anchored near map_center —
        // would jump to that raw sensor altitude instead of the real
        // ground). ground_or_raw applies the exact same "sample real
        // terrain height" step do_pan (input.tsx) already goes through, so
        // this recenter action lands exactly where panning there normally
        // would, instead of each interaction picking its own idea of
        // "where the ground is".
        set_map_center(
          useCameraStore.getState().ground_snap ? ground_or_raw(device_coord) : device_coord,
        );
      }
      set_is_follow_mode(false);
      return;
    }

    // 2. already centered? -> toggle follow mode (compass mode)
    if (!is_follow_mode) {
      // request permission and start the listener
      const success: boolean = await request_orientation();
      if (success) {
        set_is_follow_mode(true);
      } else {
        // TODO: show a toast on permission denial
        console.error("[orientation] could not start the sensor.");
      }
    } else {
      // realign north on exiting follow mode (tilt is untouched)
      set_is_follow_mode(false);
      reset_azimuth();
    }
  };

  const get_icon_style = () => {
    if (is_loading) {
      return { Icon: Loader2, className: "animate-spin opacity-40" };
    }

    if (can_show_location_control) {
      return { Icon: Locate, className: "text-white/50 scale-90" };
    }

    if (is_follow_mode) {
      const heading_deg: f64 = orientation?.heading ?? 0;
      const final_rotation_deg: f64 = heading_deg - (azimuth * 180) / Math.PI;

      return {
        Icon: Compass,
        style: { transform: `rotate(${final_rotation_deg}deg)` },
        // unified active-state color (see CircularButton's isActive ring and
        // GroundSnapToggle's text-amber-500) — this was still the old
        // sky-500 blue, left over from before that convention settled.
        className: "text-amber-500 scale-110",
      };
    }

    return {
      Icon: Navigation,
      className: "text-white/80 group-hover:rotate-[45deg]",
    };
  };

  const { Icon, className, style } = get_icon_style();

  // Mirrors get_icon_style's own state branching (see handle_action above
  // for what each state actually does) — three distinct actions live behind
  // this one button depending on current state, so a single static title
  // would describe the wrong action most of the time.
  const title = can_show_location_control
    ? t("map.controls.move_to_location")
    : is_follow_mode
    ? t("map.controls.exit_compass_mode")
    : t("map.controls.start_compass_mode");

  return (
    <CircularButton
      className="w-11 h-11"
      dark
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
