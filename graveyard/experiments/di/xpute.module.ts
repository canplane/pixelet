// // @pxl/api/xpute/xpute.module.ts (deno/hono)

// import type { Hono } from "hono";
// import type { Server } from "socket.io";

// import type { DeviceUplink } from "@xpute/spec/ipc/link.ts";
// import { CommandPort } from "@xpute/spec/control/post.ts";

// import { WSPlug } from "./plug/ws.ts";
// import { HTTPPlug } from "./plug/http.ts";
// import { RTCSignalRelay } from "./plug/rtc.signal_relay.ts";
// import { RTCSignalGateway } from "./plug/rtc.signal_gateway.ts";

// import { AppContainer, type AppModule, deps, impl } from "./container.ts";

// export const HOST_LINK = Symbol("xpute.host_link");

// export type ServerContext = {
//   root: unknown; // Hono (root)
//   app: unknown; // Hono (mounted at /api)
//   socketIoServer: unknown; // socket.io Server
// };
// export type XputeCore = {
//   app: AppContainer<ServerContext>;
// };

// // NOTE: backdoor는 export 타입에서 숨기고, mount에서만 사용
// type XputeBackdoor = {
//   http_plug: HTTPPlug;
//   ws_plug: WSPlug;
//   signal_relay: RTCSignalRelay;
//   signal_gateway: RTCSignalGateway;
// };

// export function make_xpute_module(tx: DeviceUplink): AppModule<ServerContext> {
//   return {
//     exports: [CommandPort],
//     bind(app) {
//       app.bind_value(HOST_LINK, tx);

//       // lazy singleton graph
//       @impl(CommandPort)
//       @deps(HOST_LINK)
//       class LocalCommandPort extends CommandPort {
//         constructor(tx: DeviceUplink) {
//           super(tx);
//         }
//       }

//       // 이제 bind_class(Foo)만 호출하면 Foo[IMPL]이 자동으로 선택됨
//       app.bind_package((a) => {
//         a.bind_class(CommandPort);
//         a.bind_class(HTTPPlug);
//         a.bind_class(WSPlug);
//         a.bind_class(RTCSignalRelay);
//         a.bind_class(RTCSignalGateway);
//       });
//     },
//   };
// }

// export class XputeModule {
//   static for_root(): XputeCore {
//     const app = new AppContainer<ServerContext>();
//     return { app };
//   }

//   static async mount(core: XputeCore, built: { root: Hono; app: Hono }, socketIoServer: Server): Promise<void> {
//     // standardized lifecycle entry (root/api split)
//     await core.app.mount({ root: built.root, app: built.app, socketIoServer } satisfies ServerContext);
//   }

//   static async unmount(core: XputeCore): Promise<void> {
//     await core.app.unmount();
//   }
// }
