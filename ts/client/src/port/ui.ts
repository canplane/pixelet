// @pxl/client/port/ui.ts

/** The core as the UI reaches it, besides each major's own commands, and
 * what the client's services (sys/, origin/) show the UI. The UI reaches the
 * core, sys/ and origin/ through port/ alone, and reaches the platform's
 * services (service/) directly. */

import type { u32 } from "@xpute/kit/abi/word.ts";
import type { NodeValue, NodeView } from "@xpute/kit/wire/xtp/mod.ts";
import { encode_args } from "@xpute/runtime";
import { MACHINE } from "@/abi/machine.spec.ts";
import { ui } from "@/port/core.ts";

// Where the client's boot stands, and the stop code of a core that stopped.
export { boot_state, watch_boot } from "@/port/boot.ts";
export { stop_code } from "@/sys/abort.ts";

// What the client's services show the UI: the GPU's state, the network's,
// the world's source, and where the origin serves what the UI fetches itself.
export { gpu, watch_gpu } from "@/port/gpu.ts";
export { online, watch_online } from "@/origin/online.ts";
export {
  source_state,
  source_versions,
  watch_source,
} from "@/sys/fs/source.ts";
export { asset_url, get, href } from "@/origin/route.ts";
export { SEARCH_URL } from "@/origin/route.spec.ts";
export { ASSET_FILES, PACK } from "@/origin/ingest.spec.ts";

/** The `cmd` of a development console. It sends any command by its number,
 * with its arguments as the fields of its packet, and rings the core. */
export const door = {
  cmd: (cmd: u32, ...args: NodeValue<NodeView>[]): void =>
    ui.send(cmd, encode_args(args, MACHINE.slot_bytes)),
};
