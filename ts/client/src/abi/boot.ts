// @pxl/client/abi/boot.ts

/**
 * Boots the client, bottom up, once its app has set the place (bracket.tsx):
 * the guest the place opens; xpute's host mounted on it with pixelet's drivers
 * (the roots, the program's calls, the stop report, each turn's submission) and
 * the frame pacing; the place told where to report what its sensors, its GPU,
 * its network and its visibility hear; then the kernel, by `WORLD_BOOT`. The
 * shell is no part of it: it reads where the boot stands through the port
 * (port/shell.ts), and what the kernel shows it once it has started.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { Listeners } from "@xpute/kit/lang/listeners.ts";
import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { strerror } from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import { FramePacing } from "@xpute/host/sched/frame_pacing.ts";
import { Command, encode_world_boot } from "@/abi/cmd.spec.ts";
import { declare_source } from "@/driver/namespace/source.ts";
import { namespace } from "@/driver/namespace/namespace.ts";
import { follow_refused, migrate } from "@/driver/namespace/storage.ts";
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
import { SENSOR_HEARD, sensor_start, sensor_stop } from "@/driver/sensor.ts";
import { submit_frame } from "@/driver/frame.ts";
import { abort } from "@/driver/stop.ts";
import { text_call } from "@/driver/text.ts";
import { bracket, type Config } from "@/bracket.tsx";
import { GPU_STATE, set_gpu } from "@/driver/gpu.ts";
import { world_visible } from "@/driver/visibility.ts";
import { Boot } from "@/abi/state.ts";
import { set_online } from "@/driver/namespace/online.ts";
import { kernel_snapshot, snapshot_posted } from "@/port/snapshot.ts";

let called = false;
let state = Boot.Starting;
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

/** What the shell hears the kernel raise: the snapshot it reads, and the kept
 * slots it names refused; the sensors the drivers start and stop. */
const HEARS: Partial<Record<Command, (f: BranchCursor) => void>> = {
  [Command.WORLD_SNAPSHOT]: (f) => {
    snapshot_posted(f);
    follow_refused(kernel_snapshot().kept_refused);
  },
  [Command.SENSOR_START]: sensor_start,
  [Command.SENSOR_STOP]: sensor_stop,
};

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
      console.info("[boot] the kernel started");
      // Its turns end with it.
      HOST.doorbell.stopped = () => {
        HOST.strobe.stop();
        set(Boot.Stopped);
      };
      set(Boot.Started);
    },
    (err) => {
      console.error("[boot] the kernel did not start:", err);
      set(Boot.Failed);
    },
  );
}

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
  await open_host(
    cfg,
    { fs: namespace(), program: text_call, abort },
    pacing,
    submit_frame,
  );
  bracket.sensor.hear(SENSOR_HEARD);
  HOST.doorbell.listen(port.SHELL, heard);
  bracket.gpu.boot({
    ready: () => HOST.send(port.SHELL, Command.GPU_READY),
    state: (state) => set_gpu(GPU_STATE[state]),
    wake: () => HOST.strobe.request(),
  });
  bracket.online.watch(set_online);
  bracket.visibility.watch(world_visible);
  // Before the kernel opens its kept slots, which it does from WORLD_BOOT on.
  migrate();
  const pkt = encode_world_boot(
    HOST.doorbell,
    __IS_DEV__,
    bracket.sensor.geo.has,
    bracket.sensor.orient.has,
    bracket.sensor.orient.gated,
    bracket.sensor.orient.silence,
  );
  const booted = HOST.doorbell.call(
    port.SHELL,
    Command.WORLD_BOOT,
    pkt,
    (result) => result,
  );
  if (booted < 0) throw new Error(`WORLD_BOOT refused: ${strerror(booted)}`);
  // Not awaited: a dev tile server may answer late.
  void declare_source();
}
