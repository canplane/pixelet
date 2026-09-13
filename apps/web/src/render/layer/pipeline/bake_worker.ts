// @pxl/web/render/layer/pipeline/bake_worker.ts

/**
 * The bake worker's entry: one bake context over a memory of its own, the
 * bake command behind a port, served on the worker's own message channel.
 *
 * The memory is plain `ArrayBuffer` regions, not a `WebAssembly.Memory`:
 * JavaScriptCore reports a WebAssembly memory's whole reservation against
 * the page, and this one only has to hold a bake's scratch and the mesh it
 * is writing. 16 MB regions, one kept when idle.
 */

import { BuddyAllocator } from "@xpute/core/ds/buddy.ts";
import { CommandPort } from "@xpute/core/ipc/port.ts";
import { endpoint_link, type MessageEndpoint, serve } from "@xpute/core/ipc/link.ts";

import { BakeContext } from "./chunk_mesher.ts";
import { BAKE_CMD, bake_handler } from "./bake_command.ts";

const ctx = new BakeContext(new BuddyAllocator({ region_bytes: 1 << 24, max_idle_regions: 1 }));
const port = new CommandPort();
port.on(BAKE_CMD, bake_handler(ctx));
serve(endpoint_link(globalThis as unknown as MessageEndpoint), port);
