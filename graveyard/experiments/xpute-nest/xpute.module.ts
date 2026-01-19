// // @pxl/api-nest/xpute/xpute.module.ts

// import { DynamicModule, Global, Module } from "@nestjs/common";
// import type { DeviceUplink } from "@xpute/spec/ipc/link.ts";
// import { CommandPort } from "@xpute/spec/control/post.ts";

// // adapters only
// import { WSPlug } from "./plug/ws.ts";
// import { HTTPPlug } from "./plug/http.ts";
// import { RTCSignalRelay } from "./plug/rtc.signal_relay.ts";
// import { RTCSignalGateway } from "./plug/rtc.signal_gateway.ts";

// // 런타임으로 들어온 DeviceUplink를 DI 컨테이너에 심는 “앵커 토큰"
// export const HOST_LINK = Symbol("xpute.host_link");

// @Global() // 전역 모듈 (AppModule 하위 어디서든 NestAppBridge 사용 가능)
// @Module({
//   controllers: [HTTPPlug, RTCSignalGateway],
//   providers: [WSPlug, RTCSignalRelay],
//   exports: [CommandPort],
// })
// export class XputeModule {
//   /**
//    * [Entry Point] 런타임 Host 객체 주입
//    */
//   static forRoot(tx: DeviceUplink): DynamicModule {
//     return {
//       module: XputeModule,
//       providers: [
//         { provide: HOST_LINK, useValue: tx },
//         {
//           provide: CommandPort,
//           useFactory: (tx: DeviceUplink) => new CommandPort(tx),
//           inject: [HOST_LINK],
//         },
//       ],
//     };
//   }
// }
