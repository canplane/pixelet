// @pxl/client/abi/boot.ts

/**
 * Boots the client, bottom up, once its app has set the place (bracket.tsx):
 * the guest the place opens; xpute's host mounted on it with pixelet's
 * hostcalls (sys/: the roots, the stop report), each
 * turn's submission (port/surface.ts) and the frame pacing; the place told where to report what its sensors, its GPU,
 * its network and its visibility hear; then the kernel, by `SYS_BOOT`. The
 * shell is no part of it: it reads where the boot stands through the port
 * (port/shell.ts), and what the kernel shows it once it has started.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { Listeners } from "@xpute/kit/lang/listeners.ts";
import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { strerror } from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import { FramePacing } from "@xpute/host";
import {
  Command,
  encode_sys_boot,
  GroundStatus,
  type Snapshot,
} from "@/abi/cmd.spec.ts";
import { declare_source, source_state } from "@/sys/fs/source.ts";
import { namespace } from "@/sys/fs/namespace.ts";
import { follow_refused_files } from "@/sys/fs/refusal.ts";

import { HOST, open_host } from "@/host/mount.ts";
import * as port from "@/abi/port.spec.ts";
import { BATCH_FRAMES, MARGIN_SHARE, SETTLE_MS } from "@/abi/pacing.spec.ts";
import {
  DESKTOP_BYTES,
  TOC_AT,
  PHONE_BYTES,
  REGION_BYTES,
} from "./mem.spec.ts";
import { key } from "@/abi/toc.spec.ts";
import { stream } from "./gpu.spec.ts";
import { SENSOR_HEARD, sensor_start, sensor_stop } from "@/port/sensor.ts";
import { submit_frame } from "@/port/surface.ts";
import { abort } from "@/sys/stop.ts";
import { text_layout, text_raster } from "@/port/text.ts";
import { bracket, type Config } from "@/bracket.tsx";
import { GPU_STATE, gpu_present, gpu_ready, set_gpu } from "@/port/gpu.ts";
import { world_visible } from "@/port/sys.ts";
import {
  BOOT_FAILED,
  BOOT_LOADING,
  BOOT_STARTED,
  BOOT_STARTING,
  BOOT_STOPPED,
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

/** What the kernel sends the shell's port: the snapshot, and the files it
 * names refused; the sensors it starts and stops; and the text it calls for,
 * answered before the call returns. */
const HEARS: Partial<Record<Command, (f: BranchCursor) => void>> = {
  [Command.SHELL_SNAPSHOT]: (f) => {
    snapshot_posted(f);
    follow_refused_files(snapshot.current().refused_files);
    if (state === BOOT_LOADING && loaded(snapshot.current())) set(BOOT_STARTED);
  },
  [Command.SENSOR_START]: sensor_start,
  [Command.SENSOR_STOP]: sensor_stop,
  [Command.TEXT_LAYOUT]: text_layout,
  [Command.TEXT_RASTER]: text_raster,
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

function heard(_tag: u32, cmd: u32, f: BranchCursor | null): void {
  const take = HEARS[cmd as Command];
  if (take) take(f!);
  else
    console.warn(`[kernel] event 0x${cmd.toString(16)} with no one to hear it`);
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
  const cfg: Config = {
    bytes: bracket.device.handheld ? PHONE_BYTES : DESKTOP_BYTES,
    region: REGION_BYTES,
    toc_at: TOC_AT,
    gpu_stream: key.GPU_STREAM,
    gpu_head: stream.HEAD,
  };
  // Turns paced to the display, its numbers the kernel's own (pacing.json).
  const pacing = new FramePacing(
    {
      margin_share: MARGIN_SHARE,
      batch_frames: BATCH_FRAMES,
      settle_ms: SETTLE_MS,
    },
    (run) => requestAnimationFrame(run),
  );
  await open_host(cfg, {
    sys: { fs: namespace(), abort },
    ports: { [port.SHELL]: heard },
    policy: pacing,
    submit: submit_frame,
    // Its turns end with it.
    stopped: () => set(BOOT_STOPPED),
  });
  bracket.sensor.hear(SENSOR_HEARD);
  bracket.gpu.boot({
    ready: gpu_ready,
    state: (state) => set_gpu(GPU_STATE[state]),
    wake: () => HOST.wake(),
  });
  bracket.online.watch(set_online);
  bracket.visibility.watch(world_visible);
  const pkt = encode_sys_boot(
    HOST.doorbell,
    __IS_DEV__,
    bracket.sensor.geo.has,
    bracket.sensor.orient.has,
    bracket.sensor.orient.gated,
    bracket.sensor.orient.silence,
  );
  const booted = HOST.call(
    port.SHELL,
    Command.SYS_BOOT,
    pkt,
    (result) => result,
  );
  // A kernel that stopped in its boot is told by `stopped`, its stop code read.
  if (booted === Errno.ENOTRECOVERABLE) return;
  if (booted < 0)
    throw new Error(`the kernel refused its boot: ${strerror(booted)}`);
  console.info("[boot] the kernel started");
  const declared = declare_source();
  await gpu_present();
  await declared;
}
