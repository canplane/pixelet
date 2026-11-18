// @pxl/client/ui/feature/world/toast/controller.tsx

import { useEffect, useState, useSyncExternalStore } from "react";

import { messageKey } from "@/ui/shared/locale/message-key.ts";
import {
  type Message,
  MESSAGE_WORLD_READ_FAILING,
} from "@/abi/overlay.enum.ts";
import {
  type SensorStatus,
  SENSOR_STATUS_LOADING,
} from "@/service/core/sensor/reading.ts";
import { GPU_WAITING, type Gpu } from "@/service/sys/gpu.ts";
import { bootState, watchBoot, BOOT_STARTED } from "@/service/sys/boot.ts";

import { dismissToast, showToast, type ToastSpec, ToastTone } from "./store.ts";
import { useGpu, useCore, useOnline, useSensor } from "@/ui/shared/watch.ts";

/** The reports that can each show a toast. Among toasts of one tone, a lower
 * number ranks higher: a cause comes before what it holds up, the device's
 * report before the server's, and the world's before a sensor's. */
const GPU = 0;
const NETWORK = 1;
const SERVER = 2;
const WORLD = 3;
const LOCATION = 4;
type Report =
  typeof GPU | typeof NETWORK | typeof SERVER | typeof WORLD | typeof LOCATION;

/** The ready word is shown once, when the first world and its companion are
 * on screen. */
const UNSAID = 0;
const SAYING = 1;
const SAID = 2;
type Greeting = typeof UNSAID | typeof SAYING | typeof SAID;

const READY_MS = 3000;

/** How long the server's toast stays after its cause clears. Without it, a
 * server that fails one read and answers the next would show and hide the
 * toast every turn, and the stack would keep changing its front card. The
 * value is a product choice. */
const SERVER_HOLD_MS = 3000;

function useToast(id: Report, spec: ToastSpec | null, holdMs = 0): void {
  const key = spec?.key ?? null;
  const tone = spec?.tone ?? null;
  useEffect(() => {
    if (key !== null && tone !== null) {
      showToast(id, { key, tone });
      return;
    }
    if (holdMs === 0) {
      dismissToast(id);
      return;
    }
    const timer = setTimeout(() => dismissToast(id), holdMs);
    return () => clearTimeout(timer);
  }, [id, key, tone, holdMs]);
}

const error = (key: string): ToastSpec => ({ key, tone: ToastTone.ERROR });
const loading = (key: string): ToastSpec => ({ key, tone: ToastTone.LOADING });

function location(
  status: SensorStatus,
  failure: string | null,
): ToastSpec | null {
  if (failure !== null) return error(failure);
  return status === SENSOR_STATUS_LOADING
    ? loading("toast.location_checking")
    : null;
}

/** The GPU's toast shows while the client asks again for a GPU, because none
 * answered or the one drawing was lost. */
function gpuError(gpu: Gpu): ToastSpec | null {
  return gpu === GPU_WAITING ? error("toast.gpu_unstable") : null;
}

function serverError(stall: Message | null): ToastSpec | null {
  return stall === MESSAGE_WORLD_READ_FAILING ? error(messageKey(stall)) : null;
}

function world(
  outside: boolean,
  shown: boolean,
  companionLoading: boolean,
  greeting: Greeting,
): ToastSpec | null {
  if (outside) return error("toast.outside");
  if (!shown) return loading("toast.world_loading");
  if (companionLoading) return loading("toast.companion_loading");
  return greeting === SAYING
    ? { key: "toast.world_ready", tone: ToastTone.READY }
    : null;
}

/** Every toast is derived from what the core and the client report now. Each
 * source of an error has one toast. */
export function ToastController() {
  const locationStatus = useSensor((l) => l.state.status);
  const locationError = useSensor((l) => l.state.error);
  useToast(LOCATION, location(locationStatus, locationError));

  const gpu = useGpu();
  const stall = useCore((s) => s.stall);
  useToast(GPU, gpuError(gpu));
  useToast(NETWORK, useOnline() ? null : error("toast.network_unstable"));
  useToast(SERVER, serverError(stall), SERVER_HOLD_MS);

  const outside = useCore((s) => s.pivot.outside);
  const shown = useCore((s) => s.pivot.ground_shown);
  const companionLoading = useCore((s) => !s.companion_ready);
  const [greeting, setGreeting] = useState<Greeting>(UNSAID);
  // The cover hides the toasts until the app shows.
  const shows = useSyncExternalStore(watchBoot, bootState) === BOOT_STARTED;
  const ready = shows && !outside && shown && !companionLoading;
  useEffect(() => {
    if (ready && greeting === UNSAID) setGreeting(SAYING);
    // A ready word cut short is not shown again.
    if (!ready && greeting === SAYING) setGreeting(SAID);
  }, [ready, greeting]);
  useEffect(() => {
    if (greeting !== SAYING) return;
    const timer = setTimeout(() => setGreeting(SAID), READY_MS);
    return () => clearTimeout(timer);
  }, [greeting]);
  useToast(WORLD, world(outside, shown, companionLoading, greeting));

  return null;
}
