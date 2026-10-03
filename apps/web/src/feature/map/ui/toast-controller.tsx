// @pxl/web/feature/map/ui/toast-controller.tsx

import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

import { messageKey as locationMessageKey } from "@/locale/message-key.ts";
import { DevicePoseStatus, MapStatus } from "@/kernel/abi/cmd.spec.ts";
import { useKernel } from "@/kernel/shell.ts";

import { toastDismiss, toastShow, ToastTone } from "@/shared/ui/toast/store.ts";

const enum Report {
  Location = 0,
  World = 1,
}

const READY_MS = 3000;

export function ToastController() {
  const { t } = useTranslation();
  // A ref: react-i18next's new `t` on a language change re-showed toasts.
  const tRef = useRef(t);
  tRef.current = t;

  const locationStatus = useKernel((s) => s.location.status);
  // Translated here, not where the error fires, so a language change follows.
  const locationError = useKernel((s) => s.location.error);
  const locationErrorKey: string | null = locationError === null ? null : locationMessageKey(locationError);

  useEffect(() => {
    if (locationStatus === DevicePoseStatus.Loading) {
      toastShow(Report.Location, { message: tRef.current("toast.location_checking"), tone: ToastTone.Loading });
    } else if (locationStatus === DevicePoseStatus.Error) {
      toastShow(Report.Location, { message: tRef.current(locationErrorKey ?? "toast.location_weak_signal"), tone: ToastTone.Error });
    } else {
      toastDismiss(Report.Location);
    }
  }, [locationStatus, locationErrorKey]);

  // Only the first world is announced: after it, the map streams in as one
  // moves, and a load per area is no news.
  const mapStatus = useKernel((s) => s.map.status);
  const companionLoading = useKernel((s) => !s.companion.person_ready || !s.companion.pet_ready);
  const announced = useRef(false);

  useEffect(() => {
    if (announced.current) return;
    if (mapStatus === MapStatus.Loading || companionLoading) {
      const key = mapStatus === MapStatus.Loading ? "toast.map_loading" : "toast.companion_loading";
      toastShow(Report.World, { message: tRef.current(key), tone: ToastTone.Loading });
    } else if (mapStatus === MapStatus.Success) {
      announced.current = true;
      toastShow(Report.World, { message: tRef.current("toast.world_ready"), tone: ToastTone.Ready });
      // Not the effect's cleanup, which the next status would run before it fires.
      setTimeout(() => toastDismiss(Report.World), READY_MS);
    }
  }, [mapStatus, companionLoading]);

  return null;
}
