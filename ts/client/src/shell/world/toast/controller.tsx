// @pxl/client/shell/world/toast/controller.tsx

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { messageKey } from "@/shell/locale/message-key.ts";
import { GroundStatus, Message, SensorStatus } from "@/abi/cmd.spec.ts";
import { GPU_WAITING, type Gpu } from "@/abi/state.ts";

import { dismissToast, showToast, ToastTone } from "./store.ts";
import { useGpu, useKernel, useOnline } from "@/shell/store/watch.ts";

/** Each report's toast; of one tone, by rank: a cause before what it holds
 * up, the device's before the server's, the world's before a sensor's. */
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

/** The ready word: said once, as the first world and its companions are on screen. */
const UNSAID = 0;
const SAYING = 1;
const SAID = 2;
type Greeting = typeof UNSAID | typeof SAYING | typeof SAID;

const READY_MS = 3000;

/** How long the server's toast stays once its cause clears: a server failing
 * one read and answering the next would show and hide it each turn, the
 * stack trading its front card. A product number. */
const SERVER_HOLD_MS = 3000;

/** A toast's words, as a locale key. */
interface Said {
  key: string;
  tone: ToastTone;
}

/** The words, as `t` reads them now: react-i18next's new `t` on a language change re-showed toasts. */
function useWords(): (key: string) => string {
  const { t } = useTranslation();
  const tRef = useRef(t);
  tRef.current = t;
  return useRef((key: string) => tRef.current(key)).current;
}

/** A state's toast: shown while `said` is, gone with it, `holdMs` later. */
function useSaid(id: Report, said: Said | null, holdMs = 0): void {
  const words = useWords();
  const key = said?.key ?? null;
  const tone = said?.tone ?? null;
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

const error = (key: string): Said => ({ key, tone: ToastTone.Error });
const loading = (key: string): Said => ({ key, tone: ToastTone.Loading });

/** A timeout is only the page's, on a prompt no one answered yet (web sensor.ts
 * `PROMPT_MS`), which is no failure to report. */
function location(status: SensorStatus, failure: Message | null): Said | null {
  if (failure === Message.GeolocationTimeout) return null;
  if (failure !== null) return error(messageKey(failure));
  return status === SensorStatus.Loading
    ? loading("toast.location_checking")
    : null;
}

/** The GPU's: none answered yet, or the one drawing is lost. */
function gpuError(gpu: Gpu): Said | null {
  return gpu === GPU_WAITING ? error("toast.gpu_unstable") : null;
}

/** The server's: the reads that reach it failing. */
function serverError(stall: Message | null): Said | null {
  return stall === Message.WorldReadFailing ? error(messageKey(stall)) : null;
}

/** The world's one toast: none there, or it coming, then its companions, then the ready word once. */
function world(
  outside: boolean,
  shown: boolean,
  companionLoading: boolean,
  greeting: Greeting,
): Said | null {
  if (outside) return error("toast.outside");
  if (!shown) return loading("toast.world_loading");
  if (companionLoading) return loading("toast.companion_loading");
  return greeting === SAYING
    ? { key: "toast.world_ready", tone: ToastTone.Ready }
    : null;
}

/** Each toast is derived from what the kernel and the client say now; an error is one toast a source. */
export function ToastController() {
  const locationStatus = useKernel((s) => s.location.status);
  const locationError = useKernel((s) => s.location.err);
  useSaid(LOCATION, location(locationStatus, locationError));

  const orientationError = useKernel((s) => s.orient.err);
  useSaid(
    ORIENTATION,
    orientationError !== null ? error(messageKey(orientationError)) : null,
  );

  const gpu = useGpu();
  const stall = useKernel((s) => s.stall);
  useSaid(GPU, gpuError(gpu));
  useSaid(NETWORK, useOnline() ? null : error("toast.network_unstable"));
  useSaid(SERVER, serverError(stall), SERVER_HOLD_MS);

  const outside = useKernel((s) => s.pivot.outside);
  const shown = useKernel((s) => s.pivot.status === GroundStatus.Success);
  const companionLoading = useKernel(
    (s) => !s.companion.person_ready || !s.companion.pet_ready,
  );
  const [greeting, setGreeting] = useState<Greeting>(UNSAID);
  const ready = !outside && shown && !companionLoading;
  useEffect(() => {
    if (ready && greeting === UNSAID) setGreeting(SAYING);
    // Cut short, it is not said again.
    if (!ready && greeting === SAYING) setGreeting(SAID);
  }, [ready, greeting]);
  useEffect(() => {
    if (greeting !== SAYING) return;
    const timer = setTimeout(() => setGreeting(SAID), READY_MS);
    return () => clearTimeout(timer);
  }, [greeting]);
  useSaid(WORLD, world(outside, shown, companionLoading, greeting));

  return null;
}
