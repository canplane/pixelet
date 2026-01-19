// @pxl/api/xpute.manifest.ts

import { DevicePort, WorkerID, type XputeManifest } from "@xpute/spec/ipc/topology.ts";
import { Node } from "@xpute/spec/rpc/node.ts";

import * as env from "@/lib/env/mod.ts";

const api_base_url: string = env.must("API_BASE_URL");

export default {
  worker: () => import("./xpute/worker/spawn.ts"),
  guest: () => import("./xpute/mod.ts"),
  devices: [
    [WorkerID.MAIN, []],
    [
      WorkerID.CORE,
      [
        [DevicePort.ENGINE, { type: "JS", load: () => import("@pxl/engine/lib.ts") }],
        [DevicePort.ENGINE_SPARE, { type: "JS", load: () => import("@pxl/engine_spare/lib.ts") }],
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
