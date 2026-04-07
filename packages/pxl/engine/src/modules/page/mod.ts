// @pxl/engine/src/modules/page/mod.ts

import type { Bytes } from "@xpute/spec/lib/kit/array.ts";

import { type IVec, ivec } from "@pxl/spec/space/pvf3/mod.ts";

import type { DeviceEnvironment } from "@xpute/spec/ipc/link.ts";
import { Vector } from "@xpute/spec/control/vec.ts";
import { Command } from "@xpute/spec/control/cmd.ts";
import { Node } from "@xpute/spec/rpc/node.ts";

import { bake_terrain } from "@pxl/bake/terrain/bake.ts";

// ============ Client ============
// Receives page demands, fetches terrain from the server, and relays results back to the local shell.

const load_client = (env: DeviceEnvironment): void => {
  const { signal, post } = env;

  post.on<IVec[]>(Command.TILE, (args, _body) => {
    const demands: IVec[] = args;

    // Request each page independently and relay results as they arrive.
    demands.forEach(async (page_idx) => {
      try {
        const terrain_encoded: Bytes | null = await signal.req.to(Node.SERVER).send(Vector.TILE, [page_idx]).wait();
        void post.req.unack().send(Command.TILE, [page_idx], terrain_encoded); // relay
      } catch (err) {
        console.error(`[${Command.TILE}/${ivec.repr(page_idx)}] core: failed to stream tile:`, err);
        void post.req.unack().send(Command.TILE, [page_idx], null);
      }
    });
  });
};

// ============ Server ============
// Fetches source terrain payloads and returns baked terrain pages.

const load_server = (env: DeviceEnvironment): void => {
  const { signal, post } = env;

  signal.on<[IVec]>(Vector.TILE, async (_sel, args, _body) => {
    const [page_idx] = args;

    const terrain_src: Bytes | null = await post.req.send(Command.TERRAIN_SOURCE, [page_idx]).wait();
    if (!terrain_src) throw new Error(`[${Command.TERRAIN_SOURCE}/${ivec.repr(page_idx)}] core: fetching terrain source failed`);

    return bake_terrain(page_idx, terrain_src);
  });
};

/** Module entry point. */
export function load(env: DeviceEnvironment) {
  switch (__ROLE__) {
    case "server":
      return load_server(env);
    case "client":
      return load_client(env);
    default:
      throw new Error(`what do you do?: ${__ROLE__}`);
  }
}
