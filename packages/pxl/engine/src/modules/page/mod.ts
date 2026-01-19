// @pxl/engine/modules/page/mod.ts

import type { Bytes } from "@xpute/spec/lib/kit/array.ts";

import { type IVec, ivec } from "@pxl/spec/space/pvf3/mod.ts";

import type { DeviceEnvironment } from "@xpute/spec/ipc/link.ts";
import { Vector } from "@xpute/spec/control/vec.ts";
import { Command } from "@xpute/spec/control/cmd.ts";
import { Node } from "@xpute/spec/rpc/node.ts";

import { bake_terrain } from "@pxl/bake/terrain/bake.ts";

// ============ Client ============
// 페이지 폴트 발생 시 서버에서 데이터를 긁어와 로컬 pt에 매핑

const client = (env: DeviceEnvironment): void => {
  const { signal, post } = env;

  post.on<IVec[]>(Command.TILE, (args, _body) => {
    const demands: IVec[] = args; // 요청받은 좌표 배열

    // 🚀 병렬로 서버에 요청하되, 각각 완료되는 대로 클라이언트에 쏴줌 (Streaming)
    demands.forEach(async (page_idx) => {
      try {
        // 서버에 개별 페이지 요청
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
// DB/Cloud에서 레이어별 데이터를 읽어 병합 후 전송

const server = (env: DeviceEnvironment): void => {
  const { signal, post } = env;

  signal.on<[IVec]>(Vector.TILE, async (_sel, args, _body) => {
    const [page_idx] = args;

    const terrain_src: Bytes | null = await post.req.send(Command.TERRAIN_SOURCE, [page_idx]).wait();
    if (!terrain_src) throw new Error(`[${Command.TERRAIN_SOURCE}/${ivec.repr(page_idx)}] core: fetching terrain source failed`);

    return bake_terrain(page_idx, terrain_src);
  });
};

/** Module Entry */
export function load(env: DeviceEnvironment) {
  switch (__ROLE__) {
    case "server":
      return server(env);
    case "client":
      return client(env);
    default:
      throw new Error(`what do you do?: ${__ROLE__}`);
  }
}
