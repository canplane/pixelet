// // @pxl/api/xpute/mod.ts

// import { make_xpute_module, XputeModule } from "./xpute.module.ts";
// import { main } from "@/main.ts";

// import type { GuestBootstrapper } from "@xpute/spec/ipc/driver.ts";

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

//   // xpute = recipe (host captured here, not in main)
//   const xpute = make_xpute_module(host);

//   // boot userland: build Root graph + http surface (do not listen yet)
//   const { core, built, socketIoServer, listen } = main(xpute);

//   // mount ctx drives BOTH: backdoor plugs + AppRouteHub(route registration)
//   await XputeModule.mount(core, built as any, socketIoServer);

//   // online (serve call lives in main.ts)
//   listen();

//   // export surface (after root install)
//   const exported = core.app.exported();
//   const post = exported.get(CommandPort);

//   // optional: container lifecycle (if you want plug mount/unmount to run deterministically)
//   // - XputeModule.mount() 내부가 core.app.mount(...)를 호출하니까,
//   //   io 생성 후에 mount() 타이밍만 보장하면 됨.

//   // transports (internal)
//   const xports = {
//     [TransportKind.CENTRAL]: new CentralTransport(core.app.get(WSPlug), core.app.get(HTTPPlug), 10_000),
//     [TransportKind.MESH]: new MeshTransport(core.app.get(RTCSignalRelay)),
//   } as const satisfies Transports;

//   const net_gateway = new NetGateway(net_uplink, xports, {
//     hello_cookie_secret: Deno.env.get("HELLO_COOKIE_SECRET_B64URL"),
//   });

//   return { post, net_gateway };
// };

// export default bootstrap;
