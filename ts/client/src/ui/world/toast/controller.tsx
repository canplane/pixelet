// @pxl/client/ui/world/toast/controller.tsx

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { messageKey } from "@/ui/locale/message-key.ts";
import { GroundStatus, Message, SensorStatus } from "@/abi/cmd.spec.ts";
import { GPU_WAITING, type Gpu } from "@/abi/state.ts";

import { dismissToast, showToast, ToastTone } from "./store.ts";
import { useGpu, useCore, useOnline } from "@/ui/watch.ts";

/** The reports that can each show a toast. Among toasts of one tone, a lower
 * number ranks higher: a cause comes before what it holds up, the device's
 * report before the server's, and the world's before a sensor's. */
const GPU = 0;
const NETWORK = 1;
const SERVER = 2;
const WORLD = 3;
const LOCATION = 4;
const ORIENTATION = 5;
type Report =
  | typeof GPU
  | typeof NETWORK
  | typeof SERVER
  | typeof WORLD
  | typeof LOCATION
  | typeof ORIENTATION;

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

/** A toast, with its words given as a locale key. */
interface Notice {
  key: string;
  tone: ToastTone;
}

/** A function that stays the same across renders and reads words with the
 * current `t`. react-i18next makes a new `t` on a language change, and a new
 * function would show every toast again. */
function useWords(): (key: string) => string {
  const { t } = useTranslation();
  const tRef = useRef(t);
  tRef.current = t;
  return useRef((key: string) => tRef.current(key)).current;
}

/** Shows report `id`'s toast while `notice` is set, and dismisses it `holdMs`
 * after `notice` clears. */
function useNotice(id: Report, notice: Notice | null, holdMs = 0): void {
  const words = useWords();
  const key = notice?.key ?? null;
  const tone = notice?.tone ?? null;
  useEffect(() => {
    if (key !== null && tone !== null) {
      showToast(id, { message: words(key), tone });
      return;
    }
    if (holdMs === 0) {
      dismissToast(id);
      return;
    }
    const timer = setTimeout(() => dismissToast(id), holdMs);
    return () => clearTimeout(timer);
  }, [id, key, tone, words, holdMs]);
}

const error = (key: string): Notice => ({ key, tone: ToastTone.Error });
const loading = (key: string): Notice => ({ key, tone: ToastTone.Loading });

/** Only a page times out a location request, when its permission prompt has
 * gone unanswered for `PROMPT_MS` (service/platform/sensor.ts). That is not a
 * failure, so it shows no toast. */
function location(
  status: SensorStatus,
  failure: Message | null,
): Notice | null {
  if (failure === Message.GeolocationTimeout) return null;
  if (failure !== null) return error(messageKey(failure));
  return status === SensorStatus.Loading
    ? loading("toast.location_checking")
    : null;
}

/** The GPU's toast shows while the client asks again for a GPU, because none
 * answered or the one drawing was lost. */
function gpuError(gpu: Gpu): Notice | null {
  return gpu === GPU_WAITING ? error("toast.gpu_unstable") : null;
}

function serverError(stall: Message | null): Notice | null {
  return stall === Message.WorldReadFailing ? error(messageKey(stall)) : null;
}

/** The world has one toast, which goes through its states in order: an area
 * the world does not cover, the world loading, the companion loading, and the
 * ready word once. */
function world(
  outside: boolean,
  shown: boolean,
  companionLoading: boolean,
  greeting: Greeting,
): Notice | null {
  if (outside) return error("toast.outside");
  if (!shown) return loading("toast.world_loading");
  if (companionLoading) return loading("toast.companion_loading");
  return greeting === SAYING
    ? { key: "toast.world_ready", tone: ToastTone.Ready }
    : null;
}

/** Every toast is derived from what the core and the client report now. Each
 * source of an error has one toast. */
export function ToastController() {
  const locationStatus = useCore((s) => s.location.status);
  const locationError = useCore((s) => s.location.err);
  useNotice(LOCATION, location(locationStatus, locationError));

  const orientationError = useCore((s) => s.orient.err);
  useNotice(
    ORIENTATION,
    orientationError !== null ? error(messageKey(orientationError)) : null,
  );

  const gpu = useGpu();
  const stall = useCore((s) => s.stall);
  useNotice(GPU, gpuError(gpu));
  useNotice(NETWORK, useOnline() ? null : error("toast.network_unstable"));
  useNotice(SERVER, serverError(stall), SERVER_HOLD_MS);

  const outside = useCore((s) => s.pivot.outside);
  const shown = useCore((s) => s.pivot.status === GroundStatus.Success);
  const companionLoading = useCore(
    (s) => !s.companion.person_ready || !s.companion.pet_ready,
  );
  const [greeting, setGreeting] = useState<Greeting>(UNSAID);
  const ready = !outside && shown && !companionLoading;
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
  useNotice(WORLD, world(outside, shown, companionLoading, greeting));

  return null;
}
