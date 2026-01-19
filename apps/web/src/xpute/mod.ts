// @pxl/web/xpute/mod.ts

import { CommandPort } from "@xpute/spec/control/post.ts";
import { NetGateway } from "@xpute/core/net/gateway.ts";
import { TransportKind, type Transports } from "@xpute/spec/net/xport/mod.ts";
import type { XputeContext } from "@xpute/spec/ipc/link.ts";
import type { GuestBootstrapper } from "@xpute/core/ipc/worker/contract.ts";

import { CentralTransport } from "@xpute/core/net/xport/central.ts";
import { WSPlug } from "./plug/ws.ts";
import { HTTPPlug } from "./plug/http.ts";

import { MeshTransport } from "@xpute/core/net/xport/mesh.ts";
import { RTCPlug } from "./plug/rtc.ts";

import { set_xpute } from "@/xpute/context.ts";

import { main } from "@/main.tsx";

const bootstrap: GuestBootstrapper = (tx, net_uplink) => {
  console.debug("xpute: guest starting...");

  // TODO(ops):
  // - testnet-only mode switch (verbose logs, strict seed allowlist, optional outbound disable)
  // - incident kill switch plumbing (disable net_gateway send / disable selected xport kinds)
  // - expose minimal health/debug panel (dev only): link states, hello failures, peer count

  // ---- env ----
  const post = new CommandPort(tx);

  const xpute: XputeContext = { post };

  // ---- transports ----
  const ws = new WSPlug();
  const http = new HTTPPlug();
  const rtc = new RTCPlug();

  set_xpute(xpute);

  // ---- network gateway ----
  const xports = {
    [TransportKind.CENTRAL]: new CentralTransport(ws, http, 10_000),
    [TransportKind.MESH]: new MeshTransport(rtc),
  } as const satisfies Transports;

  const net_gateway = new NetGateway(net_uplink, xports);

  // handover
  return { post, net_gateway, start: main };
};

export default bootstrap;
