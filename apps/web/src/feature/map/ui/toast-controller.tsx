// @pxl/web/feature/map/ui/toast-controller.tsx

import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

import { messageKey as locationMessageKey } from "@/locale/message-key.ts";
import { DevicePoseStatus, MapStatus } from "@/kernel/abi/cmd.spec.ts";
import { useKernel } from "@/kernel/shell.ts";

import { toastDismiss, toastShow, ToastTone } from "@/shared/ui/toast/store.ts";

/** The reports the map keeps up, each one toast that replaces itself. */
const enum Report {
  Location = 0,
  World = 1,
  Orientation = 2,
}

const READY_MS = 3000;

export function ToastController() {
  const { t } = useTranslation();
  // A ref: react-i18next's new `t` on a language change re-showed toasts.
  const tRef = useRef(t);
  tRef.current = t;

  const locationStatus = useKernel((s) => s.location.status);
  // A failure is its message; translated here, so a language change follows.
  const locationError = useKernel((s) => s.location.error);
  const orientationError = useKernel((s) => s.orientation.error);

  useEffect(() => {
    if (locationError !== null) {
      toastShow(Report.Location, { message: tRef.current(messageKey(locationError)), tone: ToastTone.Error });
    } else if (locationStatus === DevicePoseStatus.Loading) {
      toastShow(Report.Location, { message: tRef.current("toast.location_checking"), tone: ToastTone.Loading });
    } else {
      toastDismiss(Report.Location);
    }
  }, [locationStatus, locationError]);

  useEffect(() => {
    if (orientationError !== null) {
      toastShow(Report.Orientation, { message: tRef.current(messageKey(orientationError)), tone: ToastTone.Error });
    } else {
      toastDismiss(Report.Orientation);
    }
  }, [orientationError]);

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
