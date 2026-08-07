// @pxl/web/modules/map/ui/toast/toast_controller.tsx

import { useEffect, useRef } from "react";
import { toast } from "sonner";
import { AnimatePresence, motion } from "framer-motion";
import { CheckCircle2, Loader2, MapPinOff } from "lucide-react";

import { DevicePoseStatus, useDeviceLocationStore } from "@/modules/device/pose/store.ts";
import { MapStatus, useMapStore } from "@/modules/map/store.ts";

import { Toast } from "@/shared/ui/toast.tsx";

export function MapToastController() {
  const location_status: DevicePoseStatus = useDeviceLocationStore((state) => state.status);
  const location_error_msg: string | null = useDeviceLocationStore((state) => state.error_msg);

  const map_status: MapStatus = useMapStore((state) => state.map_status);

  function get_toast_cfg() {
    // shared loading icon
    const LoadingIcon = (
      <motion.div
        key="loading-icon"
        initial={{ opacity: 0, scale: 0.8 }}
        animate={{ opacity: 1, scale: 1 }}
        exit={{ opacity: 0, scale: 0.5 }}
        transition={{ duration: 0.2 }}
      >
        <Loader2 className="w-4 h-4 animate-spin-slow stroke-[2.5px]" />
      </motion.div>
    );

    // shared success icon
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

    const isError = location_status === DevicePoseStatus.ERROR || map_status === MapStatus.ERROR;
    if (isError) {
      return {
        id: "location-error", // separate id so an error toast never gets replaced
        mode: "error" as const,
        title: location_error_msg || "GPS Signal Weak",
        // description: "Signal",
        // subDescription: "실외로 이동하거나 잠시만 기다려주세요.",
        statusColor: "bg-red-500",
        icon: <MapPinOff className="w-4 h-4 text-red-500" />,
        // 5000 -> 7000: the pill spends its first ~450ms collapsed
        // (icon-only, before wasExpanded flips true in Toast), so the
        // actual readable-text window was only ~4.5s — felt short for
        // an error message the user might not glance at immediately.
        duration: 7000,
        backgroundColor: "bg-red-50/85",
        className: "border-red-200 shadow-[0_6px_20px_rgba(220,38,38,0.15)]",
      };
    }

    if (location_status === DevicePoseStatus.LOADING) {
      return {
        id: "map-status-island",
        mode: "status" as const,
        status: "loading" as const,
        title: "당신의 위치를 찾는 중입니다",
        // description: "MAP",
        // subDescription: "정확한 위치를 확인하고 있어요.",
        statusColor: "bg-[#34C759]/30",
        icon: LoadingIcon,
        duration: Infinity,
      };
    }

    /**
     * location found, map still loading.
     *
     * `!== SUCCESS`, not `=== LOADING`: on first init, location_status can
     * flip to READY for one render while map_status is still IDLE (before
     * useMapBootstrap's own effect catches up). That (READY, IDLE)
     * combination used to match no branch, so the effect dismissed the
     * just-shown loading toast and immediately recreated it under the same
     * id a beat later — landing in Sonner's 200ms removal grace period and
     * not reliably reappearing. Treating "READY but not yet SUCCESS" as
     * continuous with LOADING closes that gap: the controller never has a
     * reason to dismiss before the real SUCCESS/ERROR outcome is known.
     */
    if (location_status === DevicePoseStatus.READY && map_status !== MapStatus.SUCCESS) {
      return {
        id: "map-status-island",
        mode: "status" as const,
        status: "loading" as const,
        title: "주변 공간을 불러오고 있어요",
        statusColor: "bg-[#34C759]/30",
        icon: LoadingIcon,
        duration: Infinity,
      };
    }

    if (location_status === DevicePoseStatus.READY && map_status === MapStatus.SUCCESS) {
      return {
        id: "map-status-island",
        mode: "status" as const,
        status: "success" as const,
        title: "공간 동기화 완료",
        // description: "MAP",
        statusColor: "bg-[#34C759]",
        icon: SuccessIcon,
        // 2000 -> 3500: same "~450ms spent collapsed before text even
        // shows" issue as the error toast above — 2000ms left well under
        // 1.6s of actually-readable text.
        duration: 3500,
      };
    }

    return null;
  }

  /**
   * Whether we've actually shown "map-status-island" at least once.
   *
   * Sonner's toast.dismiss(id) doesn't check whether that id currently
   * exists — it arms an rAF-deferred delete signal regardless, while
   * toast.custom()/create() publishes synchronously. Every mount starts at
   * (IDLE, IDLE) -> null config -> dismiss(), then a beat later flips to
   * LOADING -> toast.custom() creates the toast — but the earlier dismiss's
   * rAF was still pending and fires right after, deleting what was just
   * created. Since every mount hits this exact order, the first loading
   * toast was wiped on every page load. Only calling dismiss() once we know
   * something's actually shown closes the gap.
   */
  const has_shown_toast_ref = useRef(false);

  useEffect(() => {
    const config = get_toast_cfg();

    // skip creating a toast for an invalid initial state (e.g. idle)
    if (!config) {
      if (has_shown_toast_ref.current) {
        toast.dismiss("map-status-island");
        has_shown_toast_ref.current = false;
      }
      return;
    }

    has_shown_toast_ref.current = true;

    // toast.custom() with the same id swaps content smoothly instead of remounting.
    toast.custom(
      () => (
        <Toast
          id={config.id}
          title={config.title}
          // description={config.description}
          // subDescription={config.subDescription}
          statusColor={config.statusColor}
          icon={
            <div className="relative flex items-center justify-center w-4 h-4">
              <AnimatePresence mode="wait" initial={false}>
                {config.icon}
              </AnimatePresence>
            </div>
          }
          backgroundColor={config.backgroundColor}
          className={config.className}
        />
      ),
      // Sonner's own duration stays Infinity always. Passing config.duration
      // through would let Sonner's own timer expire it — that path unmounts
      // Toast before it learns it's being removed, so is_removing's fade
      // never runs. toast.dismiss() instead notifies first (via rAF), then
      // waits its own 200ms before real DOM removal — enough time for the
      // fade. So duration is managed by our own timer below, always through
      // this explicit-dismiss path.
      { id: config.id, duration: Infinity },
    );

    if (config.duration === Infinity) return;
    const timer = setTimeout(() => toast.dismiss(config.id), config.duration);
    return () => clearTimeout(timer);
  }, [location_status, map_status, location_error_msg]);

  return null;
}
