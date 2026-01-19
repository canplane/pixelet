// @pxl/web/xpute.manifest.ts

import { DevicePort, WorkerID, type XputeManifest } from "@xpute/spec/ipc/topology.ts";
import { Node } from "@xpute/spec/rpc/node.ts";

import * as env from "@/lib/env/mod.ts";

const api_base_url: string = env.get("VITE_API_BASE_URL") || globalThis.location.origin;

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
    role: Node.CLIENT,
    seeds: [
      api_base_url, // CENTRAL bootstrap
      // "rtc:lobby", // MESH bootstrap (draft example)
    ],
    // advert: {
    //   mesh: {
    //     rendezvous_dial: "rtc:lobby",
    //     // signal_dial: API_SEED_URL, // draft: same endpoint may serve signaling later

    //     ice_servers: [
    //       { urls: "stun:stun.l.google.com:19302" },
    //       // turn은 나중에 여기로 추가
    //     ],
    //   },
    // },
  },
} satisfies XputeManifest;
