// @pxl/web/modules/map/ui/toast/toast_controller.tsx

import { useEffect, useRef } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { CheckCircle2, Loader2, MapPinOff } from "lucide-react";

import { DevicePoseStatus, useDeviceLocationStore } from "@/modules/device/pose/store.ts";
import { MapStatus, useMapStore } from "@/modules/map/store.ts";

import { Toast } from "@/shared/ui/toast/toast.tsx";
import { toast_custom, toast_dismiss } from "@/shared/ui/toast/store.ts";

const LoadingIcon = (
  <motion.div
    key="loading-icon"
    initial={{ opacity: 0, scale: 0.8 }}
    animate={{ opacity: 1, scale: 1 }}
    exit={{ opacity: 0, scale: 0.5 }}
    transition={{ duration: 0.2 }}
  >
    <Loader2 className="w-4 h-4 text-[#5B4636] animate-spin-slow stroke-[2.5px]" />
  </motion.div>
);

const SuccessIcon = (
  <motion.div
    key="success-icon"
    initial={{ opacity: 0, scale: 0.3, rotate: -20 }}
    animate={{ opacity: 1, scale: 1, rotate: 0 }}
    transition={{ type: "spring", damping: 12, stiffness: 260, mass: 0.8 }}
  >
    <CheckCircle2 className="w-4 h-4 text-[#34C759] stroke-[2.5px]" />
  </motion.div>
);

function StatusIcon({ children }: { children: React.ReactNode }) {
  return (
    <div className="relative flex items-center justify-center w-4 h-4">
      <AnimatePresence mode="wait" initial={false}>
        {children}
      </AnimatePresence>
    </div>
  );
}

/**
 * Two independent toasts, not one shared "map-status-island" — location and
 * map load status used to be crammed into a single id with a hand-rolled
 * priority order (map errors > map loading > map success > location errors
 * > location loading) to fake mutual exclusion between them. But they
 * genuinely aren't the same axis: map_status only ever reflects tiles
 * loading around map_center (use_tile_fetcher.ts), which moves just as much
 * from the user panning/dragging the map as it does from GPS — location
 * failing (permission denied, no signal) says nothing about whether tiles
 * can load, and use_map_bootstrap.ts's Seoul fallback means they usually
 * can. Squashing both into one slot meant only one of two simultaneously
 * true, unrelated facts could ever be shown at a time. The toast deck
 * already supports multiple simultaneous toasts (that's the whole point of
 * the card-deck stack) — there's no reason these two need to fight over one
 * spot instead of just both existing when both apply.
 */
export function MapToastController() {
  const location_status: DevicePoseStatus = useDeviceLocationStore((state) => state.status);
  const location_error_msg: string | null = useDeviceLocationStore((state) => state.error_msg);

  const map_status: MapStatus = useMapStore((state) => state.map_status);

  /** Location (GPS) — purely about whether we know where the device is, nothing about the map/tiles. */
  const shown_location_toast_ref = useRef(false);
  useEffect(() => {
    const is_loading = location_status === DevicePoseStatus.LOADING;
    const is_error = location_status === DevicePoseStatus.ERROR;

    if (!is_loading && !is_error) {
      if (shown_location_toast_ref.current) {
        toast_dismiss("location-status");
        shown_location_toast_ref.current = false;
      }
      return;
    }

    shown_location_toast_ref.current = true;
    toast_custom("location-status", ({ index, stack_size, is_removing }) => (
      <Toast
        id="location-status"
        index={index}
        stack_size={stack_size}
        is_removing={is_removing}
        message={is_error ? (location_error_msg || "위치 신호가 약합니다.") : "위치를 확인하고 있습니다."}
        statusColor={is_error ? "bg-red-500" : "bg-[#34C759]/30"}
        icon={<StatusIcon>{is_error ? <MapPinOff key="location-error-icon" className="w-4 h-4 text-red-500" /> : LoadingIcon}</StatusIcon>}
        backgroundColor={is_error ? "bg-red-50/85" : undefined}
        className={is_error ? "border-red-200 shadow-[0_6px_20px_rgba(220,38,38,0.15)]" : undefined}
      />
    ));
    // errors are state-persistent — no self-timer, dismissed only once location_status leaves LOADING/ERROR
  }, [location_status, location_error_msg]);

  /** Map/center load status — purely about whether tiles have loaded around the current map_center, nothing about GPS. */
  const shown_map_toast_ref = useRef(false);
  useEffect(() => {
    const is_loading = map_status === MapStatus.LOADING;
    const is_error = map_status === MapStatus.ERROR;
    const is_success = map_status === MapStatus.SUCCESS;

    if (!is_loading && !is_error && !is_success) {
      if (shown_map_toast_ref.current) {
        toast_dismiss("map-status");
        shown_map_toast_ref.current = false;
      }
      return;
    }

    shown_map_toast_ref.current = true;
    toast_custom("map-status", ({ index, stack_size, is_removing }) => (
      <Toast
        id="map-status"
        index={index}
        stack_size={stack_size}
        is_removing={is_removing}
        message={is_error ? "지도를 불러올 수 없습니다." : is_success ? "공간을 불러왔습니다." : "공간을 불러오고 있습니다."}
        statusColor={is_error ? "bg-red-500" : is_success ? "bg-[#34C759]" : "bg-[#34C759]/30"}
        icon={<StatusIcon>{is_error ? <MapPinOff key="map-error-icon" className="w-4 h-4 text-red-500" /> : is_success ? SuccessIcon : LoadingIcon}</StatusIcon>}
        backgroundColor={is_error ? "bg-red-50/85" : undefined}
        className={is_error ? "border-red-200 shadow-[0_6px_20px_rgba(220,38,38,0.15)]" : undefined}
      />
    ));

    if (!is_success) return; // loading/error persist until map_status actually changes
    // the pill spends its first ~450ms collapsed (icon-only, before wasExpanded
    // flips true in Toast) — 3000ms total leaves ~2.55s of actually readable text.
    const timer = setTimeout(() => toast_dismiss("map-status"), 3000);
    return () => clearTimeout(timer);
  }, [map_status]);

  return null;
}
