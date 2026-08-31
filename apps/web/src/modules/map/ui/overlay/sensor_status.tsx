// @pxl/web/modules/map/ui/overlay/sensor_status.tsx

import { Compass, Locate } from "lucide-react";
import { useTranslation } from "react-i18next";

import { DevicePoseStatus, useDeviceLocationStore, useDeviceOrientationStore } from "@/modules/device/pose/store.ts";
import { useDeviceOrientation } from "@/modules/device/pose/use_device_orientation.ts";
import { useGeolocation } from "@/modules/device/pose/use_geolocation.ts";

import { cn } from "@/shared/lib/utils/cn.ts";

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
      return "text-[#5B4636]/30";
  }
}

// DeviceOrientationEvent.requestPermission() only runs inside a direct user
// gesture on iOS Safari; use_device_orientation.ts deliberately doesn't
// auto-start there, so this control is the only route to that permission.
export function SensorStatus() {
  const { t } = useTranslation();

  const location_status = useDeviceLocationStore((state) => state.status);
  const orientation_status = useDeviceOrientationStore((state) => state.status);

  const { request_orientation } = useDeviceOrientation();
  const { start_watching } = useGeolocation();

  const can_request: boolean = location_status !== DevicePoseStatus.READY ||
    orientation_status !== DevicePoseStatus.READY;

  function on_click() {
    // Orientation first, synchronously: Safari consumes the gesture's user
    // activation, so a requestPermission() reached after an await can be
    // rejected for coming from outside it.
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
      className="flex items-center gap-2 -m-2 p-2 cursor-pointer pointer-events-auto"
      title={t("map.sensors.enable")}
      aria-label={t("map.sensors.enable")}
    >
      {icons}
    </button>
  );
}
