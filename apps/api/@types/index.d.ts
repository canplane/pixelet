// @pxl/api/../@types/index.d.ts

export {};

declare global {
  // Injected at build time (Vite `define`) — see vite.config.ts.
  const __ENV__: "deno" | "node" | "browser";
  const __APP_ENV__: string;
  const __IS_DEV__: boolean;
  const __APP_NAME__: string;
  const __ROLE__: "server" | "client";
}
