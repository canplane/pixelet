// @pxl/client/abi/boot.ts

/**
 * Boots the kernel: hands the place the guest's plan and where to tell what
 * its sensors, its GPU and its network hear; fills the host's slots with what pixelet's
 * numbers mean (the roots, the program's calls, the stop report, each frame's
 * submission and the signals); then hands the completion ring's signals to
 * whoever runs them.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import { strerror } from "@xpute/kit/status/errno.spec.ts";
import type { SysHost } from "@xpute/host/ipc/sys.ts";
import { Command, encode_world_boot, encode_world_kept_emptied, read_sensor_start, read_sensor_stop, read_world_snapshot, Sensor } from "@/abi/cmd.spec.ts";
import { declare_source } from "@/namespace/source.ts";
import { bind_roots, commit_root, open_root } from "@/namespace/roots.ts";
import { kept_refused, migrate_kept } from "@/namespace/storage.ts";
import { GUEST, GUEST_DOORBELL, HOST, open_host } from "@/host/mem.ts";
import { DESKTOP_BYTES, DIRECTORY_AT, PHONE_BYTES, REGION_BYTES } from "@/abi/mem/mem.spec.ts";
import { key } from "@/abi/directory.spec.ts";
import { stream } from "@/abi/render/gpu/api.spec.ts";
import { SENSOR_HEARD } from "@/abi/control/sensor.ts";
import { submit_frame } from "@/abi/control/input.ts";
import { text_call } from "@/abi/text.ts";
import { BUILD_NUMBER } from "@/config/deploy.ts";
import { bytes_at } from "@/host/module.ts";
import { bracket, type Plan } from "@/bracket.tsx";
import { GPU_STATE, set_gpu } from "@/host/gpu.ts";
import { set_online } from "@/host/online.ts";
import { kernel_snapshot, set_snapshot } from "@/abi/snapshot.ts";

let code: string | null = null;

/** The kernel's last report, short as a stop code: its errno's name and the
 * file and line, a published build's file a number its paths decode by. None
 * where it stopped without one. */
export function stop_code(): string | null {
  return code;
}

const abort: SysHost["abort"] = ({ errno, file, line, column, a, b }) => {
  console.error(`[kernel] stopped: ${strerror(errno)} at ${file.replace(/^\.+/, "")}:${line}:${column} (${a}, ${b})`);
  const where = `${file.split("/").pop()}:${line}`;
  code = `${strerror(errno).split(":")[0]} · ${file.startsWith("#") ? `${where} · ${BUILD_NUMBER}` : where}`;
};

function on_signal(_tag: u32, cmd: Command, f: BranchCursor | null): void {
  switch (cmd) {
    case Command.WORLD_SNAPSHOT: {
      // Too large for a ring frame; valid until the kernel writes the next, a turn away.
      const { at, len } = read_world_snapshot(f!);
      set_snapshot(bytes_at(GUEST.memory, at, len));
      // Sent after the completion ring is read: the doorbell refuses a write while it reads.
      kept_refused(kernel_snapshot().kept_refused, (slot) => queueMicrotask(() => HOST.send(Command.WORLD_KEPT_EMPTIED, encode_world_kept_emptied(GUEST_DOORBELL, slot))));
      return;
    }
    case Command.SENSOR_START:
      if (read_sensor_start(f!).sensor === Sensor.Geolocation) bracket.sensor.geo.start();
      else bracket.sensor.orient.start();
      return;
    case Command.SENSOR_STOP:
      if (read_sensor_stop(f!).sensor === Sensor.Geolocation) bracket.sensor.geo.stop();
      else bracket.sensor.orient.stop();
      return;
  }
  console.warn(`[kernel] signal 0x${cmd.toString(16)} with no one to hear it`);
}

/** Called once from shell/boot.ts, so boot order is explicit, not import order. */
export async function boot(): Promise<void> {
  // A phone gets less: JavaScriptCore counts the whole reservation as page memory.
  const plan: Plan = {
    bytes: bracket.device.handheld ? PHONE_BYTES : DESKTOP_BYTES,
    region: REGION_BYTES,
    directory_at: DIRECTORY_AT,
    gpu_stream: key.GPU_STREAM,
    gpu_head: stream.HEAD,
  };
  await open_host(plan, { open: open_root, commit: commit_root, program: text_call, abort }, submit_frame);
  bind_roots();
  bracket.sensor.hear(SENSOR_HEARD);
  GUEST_DOORBELL.signal = on_signal;
  bracket.gpu.boot({ ready: () => HOST.send(Command.GPU_READY), state: (state) => set_gpu(GPU_STATE[state]), wake: () => HOST.strobe.request() });
  bracket.online.watch(set_online);
  // Before the kernel opens its kept slots, which it does from WORLD_BOOT on.
  migrate_kept();
  const pkt = encode_world_boot(GUEST_DOORBELL, __IS_DEV__, bracket.sensor.geo.has, bracket.sensor.orient.has, bracket.sensor.orient.gated, bracket.sensor.orient.silence);
  const booted = GUEST_DOORBELL.call(Command.WORLD_BOOT, pkt, (result) => result);
  if (booted < 0) throw new Error(`WORLD_BOOT refused: ${strerror(booted)}`);
  // Not awaited: a dev tile server may answer late.
  void declare_source();
}

/** `listener` hears the kernel stop for good: its turns end with it. */
export function on_stop(listener: () => void): void {
  GUEST_DOORBELL.stopped = () => {
    HOST.strobe.stop();
    listener();
  };
}
