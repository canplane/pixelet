// @pxl/client/port/shell.ts

/** The kernel as the shell reaches it, beside each major's own commands
 * (input.ts, edit.ts, camera.ts, scene.ts, setting.ts, sensor.ts, surface.ts,
 * perf.ts, snapshot.ts, fs.ts, gpu.ts, sys.ts), and what the client's
 * services (sys/, origin/) show the UI. The shell names no runtime and no
 * service (AGENTS.md). */

import type { u32 } from "@xpute/kit/abi/word.ts";
import type { NodeValue, NodeView } from "@xpute/kit/wire/xtp/mod.ts";
import { control } from "@/runtime/kernel.ts";

// Where the client's boot stands, and the stop code of a kernel that stopped.
export { boot_state, watch_boot } from "@/abi/boot.ts";
export { stop_code } from "@/sys/abort.ts";

// What the client's services show the UI: the GPU's state, the network's, the world's
// source, and where the origin serves what the shell fetches itself.
export { gpu, watch_gpu } from "@/port/gpu.ts";
export { online, watch_online } from "@/origin/online.ts";
export {
  source_state,
  source_versions,
  watch_source,
} from "@/sys/fs/source.ts";
export { asset_url } from "@/origin/route.ts";
export { SEARCH_URL } from "@/origin/route.spec.ts";
export { ASSET_FILES, PACK } from "@/origin/ingest.spec.ts";

/** A development console's `cmd`: any command by its number, its arguments
 * its packet's fields, sent and rung. */
export const door = {
  cmd: (cmd: u32, ...args: NodeValue<NodeView>[]): void =>
    control.cmd(cmd, ...args),
};
