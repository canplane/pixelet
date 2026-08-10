// @pxl/web/modules/map/ui/overlay/sensor_status.tsx

import { Compass, Locate } from "lucide-react";

import { DevicePoseStatus, useDeviceLocationStore, useDeviceOrientationStore } from "@/modules/device/pose/store.ts";
import { cn } from "@/shared/lib/utils/cn.ts";

/**
 * Bare status icons (no CircularButton chrome — these aren't interactive,
 * just a glance-able "is the sensor actually working" readout) for the two
 * device sensors this app depends on. Deliberately separate from
 * TrackingButton, which is a single action button multiplexing several
 * different jobs (recenter / toggle follow mode) onto one icon whose shape
 * already changes with state — that's the wrong place to also answer "is
 * geolocation currently receiving fixes," a question that matters even when
 * that button isn't showing anything sensor-related at all (e.g. mid
 * free-look pan, where TrackingButton reads as a plain "recenter"
 * arrow).
 */

function status_class(status: DevicePoseStatus): string {
  switch (status) {
    case DevicePoseStatus.READY:
      return "text-emerald-500";
    case DevicePoseStatus.LOADING:
      return "text-[#5B4636]/40 animate-pulse";
    case DevicePoseStatus.ERROR:
      return "text-red-400";
    case DevicePoseStatus.IDLE:
    default:
      return "text-[#5B4636]/20";
  }
}

export function SensorStatus() {
  const location_status = useDeviceLocationStore((state) => state.status);
  const orientation_status = useDeviceOrientationStore((state) => state.status);

  return (
    <div className="flex items-center gap-2">
      <Locate
        className={cn("w-3 h-3 transition-colors duration-500", status_class(location_status))}
        strokeWidth={2.5}
      />
      <Compass
        className={cn("w-3 h-3 transition-colors duration-500", status_class(orientation_status))}
        strokeWidth={2.5}
      />
    </div>
  );
}
