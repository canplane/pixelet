// @pxl/web/modules/map/ui/overlay/sensor_status.tsx

import { Compass, Locate } from "lucide-react";
import { useTranslation } from "react-i18next";

import { DevicePoseStatus, useDeviceLocationStore, useDeviceOrientationStore } from "@/modules/device/pose/store.ts";
import { useDeviceOrientation } from "@/modules/device/pose/use_device_orientation.ts";
import { useGeolocation } from "@/modules/device/pose/use_geolocation.ts";

import { cn } from "@/shared/lib/utils/cn.ts";

/**
 * These sit in a bg-black/40 pill (top_overlays.tsx), which is what the
 * inactive states have to be legible against. They used to be #5B4636 — the
 * cozy theme's brown, meant for a light surface — at 20% and 40% opacity, and
 * on a dark pill that is indistinguishable from black: the compass simply
 * looked unpainted rather than idle.
 *
 * Read as a traffic light now, which is what these three states already are:
 * amber while acquiring, green once fixed, red on failure. Only IDLE stays
 * neutral, because "not started" is not a stage of that sequence — it is the
 * absence of one. Shared by both icons, so geolocation and orientation say
 * the same thing the same way.
 *
 * Amber-400 rather than a new orange: it is the accent this app already uses
 * for a live-but-not-settled state (see the scale bar and compass at their
 * default zoom).
 */
function status_class(status: DevicePoseStatus): string {
  switch (status) {
    case DevicePoseStatus.READY:
      return "text-emerald-500";
    case DevicePoseStatus.LOADING:
      return "text-amber-400 animate-pulse";
    case DevicePoseStatus.ERROR:
      return "text-red-400";
    case DevicePoseStatus.IDLE:
    default:
      return "text-white/30";
  }
}

/**
 * Sensor state, and the way to grant what is missing.
 *
 * It was display-only, which left iOS with no visible route to the orientation
 * permission: `DeviceOrientationEvent.requestPermission()` only runs inside a
 * direct user gesture, so use_device_orientation.ts deliberately does not
 * auto-start there. The compass therefore sat grey every session until the
 * user happened to press the tracking button — an indicator reporting a state
 * with no hint that it was the thing to press. Other engines have no such
 * restriction and go green on their own, which is why this only ever looked
 * broken on Safari.
 *
 * One target for both icons rather than two: at this size a pair of separate
 * hit areas would each be too small to land, and there is no case where
 * granting one and not the other is what someone wanted.
 */
export function SensorStatus() {
  const { t } = useTranslation();

  const location_status = useDeviceLocationStore((state) => state.status);
  const orientation_status = useDeviceOrientationStore((state) => state.status);

  const { request_orientation } = useDeviceOrientation();
  const { start_watching } = useGeolocation();

  // Nothing to ask for once both are live; the indicator goes back to being
  // just an indicator.
  const can_request: boolean = location_status !== DevicePoseStatus.READY ||
    orientation_status !== DevicePoseStatus.READY;

  function on_click() {
    // Orientation first, and synchronously. Safari consumes the gesture's user
    // activation, so a requestPermission() reached after an await may be
    // rejected for coming from outside it — geolocation has no such rule, so
    // it is the one that can afford to go second.
    request_orientation().catch((err) => console.error("[sensor_status] orientation request failed:", err));
    start_watching();
  }

  const icons = (
    <>
      <Locate
        className={cn("w-3.5 h-3.5 transition-colors duration-500", status_class(location_status))}
        strokeWidth={2.5}
      />
      <Compass
        className={cn("w-3.5 h-3.5 transition-colors duration-500", status_class(orientation_status))}
        strokeWidth={2.5}
      />
    </>
  );

  if (!can_request) return <div className="flex items-center gap-2">{icons}</div>;

  return (
    <button
      type="button"
      onClick={on_click}
      // -m-2 p-2: grows the hit area past the 14px icons without moving them,
      // the same trick the scale bar uses for its own tap target.
      className="flex items-center gap-2 -m-2 p-2 cursor-pointer pointer-events-auto"
      title={t("map.sensors.enable")}
      aria-label={t("map.sensors.enable")}
    >
      {icons}
    </button>
  );
}
