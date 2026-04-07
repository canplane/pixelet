// @pxl/api/xpute.manifest.ts

import { DevicePort, DeviceType, WorkerID, type XputeManifest } from "@xpute/spec/ipc/topology.ts";
import { Node } from "@xpute/spec/rpc/node.ts";

import * as env from "@/lib/env/mod.ts";

const api_base_url: string = env.must("API_BASE_URL");

export default {
  worker: () => import("./xpute/worker/spawn.ts"),
  wasm: {
    [DeviceType.BINDGEN]: () => import("./xpute/wasm/bindgen.ts"),
  },

  guest: () => import("./xpute/mod.ts"),
  devices: [
    [WorkerID.MAIN, []],
    [
      WorkerID.CORE,
      [
        [DevicePort.ENGINE, {
          type: DeviceType.BINDGEN,
          name: "Engine",
          js_url: new URL("./wasm/pxl-engine/pxl_engine.js", import.meta.url),
          wasm_url: new URL("./wasm/pxl-engine/pxl_engine_bg.wasm", import.meta.url),
          memory: { initial: 256, maximum: 4096 },
        }],

        [DevicePort.ENGINE_TS, {
          type: DeviceType.JS,
          name: "EngineLegacy",
          load: () => import("@pxl/engine/src/lib.ts"),
        }],
        [DevicePort.ENGINE_SPARE, {
          type: DeviceType.JS,
          name: "EngineSpare",
          load: () => import("@pxl/engine-spare/src/lib.ts"),
        }],
      ],
    ],
  ],

  net: {
    role: Node.SERVER,
    seeds: [
      // 'rtc:lobby', // optional draft test seed (usually server has no outbound seeds)
    ],
    advert: {
      central: { dial: api_base_url },
      // mesh: {
      //   rendezvous_dial: 'rtc:lobby',
      //   // signal_dial: 'https://signal.example.com',
      // },
    },
  },
} satisfies XputeManifest;
