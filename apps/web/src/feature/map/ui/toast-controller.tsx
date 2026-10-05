// @pxl/web/feature/map/ui/toast-controller.tsx

import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

import { messageKey } from "@/locale/message-key.ts";
import { DevicePoseStatus, MapStatus } from "@/kernel/abi/cmd.spec.ts";
import { useKernel } from "@/kernel/shell.ts";

import { toastDismiss, toastShow, ToastTone } from "@/shared/ui/toast/store.ts";

const enum Report {
  Location = 0,
  World = 1,
  Orientation = 2,
  Outside = 3,
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

  const outside = useKernel((s) => s.map.outside);

  useEffect(() => {
    if (outside) {
      toastShow(Report.Outside, { message: tRef.current("toast.outside"), tone: ToastTone.Error });
    } else {
      toastDismiss(Report.Outside);
    }
  }, [outside]);

  // An empty screen always says it is loading; the ready word is the first world's alone.
  const mapStatus = useKernel((s) => s.map.status);
  const companionLoading = useKernel((s) => !s.companion.person_ready || !s.companion.pet_ready);
  const announced = useRef(false);
  const loadingShown = useRef(false);
  const readyTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  useEffect(() => {
    if (mapStatus === MapStatus.Loading || (!announced.current && companionLoading)) {
      clearTimeout(readyTimer.current);
      loadingShown.current = true;
      const key = mapStatus === MapStatus.Loading ? "toast.map_loading" : "toast.companion_loading";
      toastShow(Report.World, { message: tRef.current(key), tone: ToastTone.Loading });
      return;
    }
    if (!loadingShown.current) return;
    loadingShown.current = false;
    if (announced.current) {
      toastDismiss(Report.World);
      return;
    }
    announced.current = true;
    toastShow(Report.World, { message: tRef.current("toast.world_ready"), tone: ToastTone.Ready });
    // Not the effect's cleanup: the next status would run it first.
    readyTimer.current = setTimeout(() => toastDismiss(Report.World), READY_MS);
  }, [mapStatus, companionLoading]);

  return null;
}
