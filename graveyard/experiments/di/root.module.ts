// // @pxl/api/xpute/root.module.ts

// import type { AppModule } from "./container.ts";
// import type { ServerContext } from "./xpute.module.ts";

// import { make_app_module } from "../modules/app.module.ts";

// // Root = (xpute I/O) + (app business)
// export function make_root_module(ctx: AppModule<ServerContext>): AppModule<ServerContext> {
//   return {
//     imports: [ctx, make_app_module()],
//     bind(_app) {},
//   };
// }
