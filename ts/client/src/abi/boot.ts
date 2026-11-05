// @pxl/client/abi/boot.ts

/**
 * Boots the client, bottom up, on its platform's services (services/): the
 * guest; xpute's host mounted on it with pixelet's hostcalls (sys/: the
 * roots, the stop report), each turn's submission (port/surface.ts) and the
 * frame pacing; the services told where to report what the sensors, the GPU,
 * the network and the visibility hear; then what the platform is, told the
 * kernel after the host's attach (`SENSOR_PLATFORM`, `SETTING_DEVELOPMENT`).
 * The shell is no part of it: it reads where the boot stands through the
 * port (port/shell.ts), and what the kernel shows it once it has started.
 */

import { Listeners } from "@xpute/kit/lang/listeners.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import { boot as boot_host, FramePacing } from "@xpute/host";
import {
  GroundStatus,
  shell_imports,
  type ShellImports,
  type Snapshot,
} from "@/abi/cmd.spec.ts";
import { MACHINE } from "@/abi/machine.spec.ts";
import { declare_source, source_state } from "@/sys/fs/source.ts";
import { namespace } from "@/sys/fs/namespace.ts";
import { follow_refused_files } from "@/sys/fs/refusal.ts";

import { host, kernel, set_host } from "@/host/kernel.ts";
import {
  BATCH_FRAMES,
  MARGIN_SHARE,
  PACE_HZ,
  SETTLE_MS,
} from "@/abi/pacing.spec.ts";
import { SENSOR_LISTENER, sensor_start, sensor_stop } from "@/port/sensor.ts";
import { submit_grant } from "@/port/surface.ts";
import { onabort } from "@/sys/abort.ts";
import { text_layout, text_raster } from "@/port/text.ts";
import * as device from "@/services/device.ts";
import * as guest from "@/services/guest.ts";
import * as online from "@/services/online.ts";
import * as sensor from "@/services/sensor.ts";
import * as visibility from "@/services/visibility.ts";
import { gpu } from "@/services/gpu.ts";
import { GPU_STATE, gpu_present, set_gpu } from "@/port/gpu.ts";
import {
  BOOT_FAILED,
  BOOT_LOADING,
  BOOT_STARTED,
  BOOT_STARTING,
  BOOT_TERMINATED,
  type Boot,
  SOURCE_DECLARED,
} from "@/abi/state.ts";
import { set_online } from "@/origin/online.ts";
import { snapshot_posted } from "@/port/snapshot.ts";
import * as snapshot from "@/port/snapshot.ts";

let called = false;
let state: Boot = BOOT_STARTING;
const listeners = new Listeners();

function set(next: Boot): void {
  state = next;
  listeners.notify();
}

/** Where the boot stands, read through the port. */
export function boot_state(): Boot {
  return state;
}

export const watch_boot = listeners.watch;

/** What the kernel sends the shell: the snapshot, and the files it
 * names refused; the sensors it starts and stops; and the text it calls for,
 * answered before the call returns. */
const SHELL: ShellImports = {
  shell_snapshot: (e) => {
    snapshot_posted(e);
    follow_refused_files(snapshot.current().refused_files);
    if (state === BOOT_LOADING && loaded(snapshot.current())) set(BOOT_STARTED);
  },
  sensor_start,
  sensor_stop,
  text_layout,
  text_raster,
};

/** Whether the world has come, or nothing will show of it: every pipeline made,
 * and the view drawn or past the service area. A failed location draws the view
 * at its default. */
function loaded(s: Snapshot): boolean {
  return (
    s.renderer_progress === 1 &&
    (s.pivot.status === GroundStatus.Success || s.pivot.outside)
  );
}

/** Called once by the app, after it has set the place, so boot order is
 * explicit, not import order; where it stands is `boot_state`. A second call
 * is refused. */
export function boot(): void {
  if (called) throw new InvariantError(Errno.EALREADY);
  called = true;
  open().then(
    () => {
      if (source_state() === SOURCE_DECLARED && state === BOOT_STARTING) {
        set(BOOT_LOADING);
      }
    },
    (err) => {
      console.error("[boot] the client did not start:", err);
      if (state === BOOT_STARTING) set(BOOT_FAILED);
    },
  );
}

/** Whether the client can run: settled once the kernel runs, a GPU has answered
 * and the world's source is declared; never where no GPU answers. */
async function open(): Promise<void> {
  // A phone gets less: JavaScriptCore counts the whole reservation as page memory.
  const sizes = MACHINE.mem_bytes;
  const bytes = device.handheld ? Math.min(...sizes) : Math.max(...sizes);
  // Turns paced to the display, its numbers the kernel's own (pacing.json).
  const pacing = new FramePacing(
    {
      pace_hz: PACE_HZ,
      margin_share: MARGIN_SHARE,
      batch_frames: BATCH_FRAMES,
      settle_ms: SETTLE_MS,
    },
    (run) => requestAnimationFrame(run),
  );
  const booted = await boot_host(MACHINE, {
    guest: guest.source({ bytes }),
    fs: namespace(),
    onabort,
    gpu: gpu,
    program: shell_imports(SHELL),
    policy: pacing,
    submit: submit_grant,
    // Its turns end with it.
    onterminated: () => set(BOOT_TERMINATED),
  });
  set_host(booted, pacing);
  sensor.listen(SENSOR_LISTENER);
  gpu.watch((state) => set_gpu(GPU_STATE[state]));
  online.watch(set_online);
  // The host tells the kernel it is hidden or shown, and holds the turns
  // while it is hidden.
  visibility.watch((visible) => host.visible(visible));
  // What the platform's sensors are, and whether this is a development
  // build, told after the host's attach, which the kernel starts on.
  const { geo, orient } = sensor;
  kernel.sensor_platform(geo.has, orient.has, orient.gated, orient.silence);
  kernel.setting_development(__IS_DEV__);
  const declared = declare_source();
  await gpu_present();
  await declared;
}
