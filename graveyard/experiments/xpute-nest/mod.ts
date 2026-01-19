// // @pxl/api-nest/xpute/mod.ts

// import { XputeModule } from "./xpute.module.ts";
// import { main } from "@/main.ts";

// import { GuestBootstrapper } from "@xpute/spec/ipc/driver.ts";

// import { CommandPort } from "@xpute/spec/control/post.ts";
// import { NetGateway } from "@xpute/core/net/net_gateway.ts";
// import { TransportKind, type Transports } from "@xpute/spec/net/xport/mod.ts";

// import { CentralTransport } from "@xpute/core/net/central.ts";
// import { WSPlug } from "./plug/ws.ts";
// import { HTTPPlug } from "./plug/http.ts";

// import { MeshTransport } from "@xpute/core/net/mesh.ts";
// import { RTCSignalRelay } from "./plug/rtc.signal_relay.ts";

// const bootstrap: GuestBootstrapper = async (host, net_uplink) => {
//   console.debug("🚀 booting guest...");
//   // TODO(ops):
//   // - env-driven safe mode (testnet-only, verbose logging, outbound kill switch)
//   // - startup self-checks: cookie secret presence/format, seed dial validation, clock sanity warning
//   // - security review doc pointer/version tag log on boot (which threat model this build assumes)

//   // boot userland
//   const app = await main(XputeModule.forRoot(host));

//   // post
//   const post = app.get(CommandPort);

//   // transport
//   const xports = {
//     [TransportKind.CENTRAL]: new CentralTransport(
//       app.get(WSPlug),
//       app.get(HTTPPlug),
//       10_000,
//     ),
//     [TransportKind.MESH]: new MeshTransport(app.get(RTCSignalRelay)),
//   } as const satisfies Transports;

//   // network gateway
//   const net_gateway = new NetGateway(net_uplink, xports, {
//     hello_cookie_secret: process.env.HELLO_COOKIE_SECRET_B64URL,
//   });

//   // handover
//   return { post, net_gateway };
// };

// export default bootstrap;
