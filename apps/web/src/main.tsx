// @pxl/web/main.tsx

/**
 * The page's entry: index.html loads this and nothing else.
 *
 * Three steps in order — whether this browser can run the app at all, what
 * the app needs before anything renders (the locale, the kernel's boot), then
 * the tree itself. A browser that cannot is shown why in place of the app,
 * and nothing is started: no core fetched, no world opened. A failure in any
 * step is logged rather than thrown: the page is already on screen by then,
 * and an uncaught error here would leave it blank with nothing said.
 */

import { StrictMode } from "react";
import { AppRegistry, type RootTag } from "react-native";

import { App } from "@/app/app.tsx";
import { bootstrapApp } from "@/app/bootstrap.ts";
import { bootstrapI18n } from "@/locale/i18n.ts";
import { Unsupported } from "@/platform/web/unsupported.tsx";
import { whatIsMissing } from "@/platform/web/webgpu.ts";
import "@/app/style/index.css";

// Before anything else, so a failure in the boot below is itself forwarded.
if (import.meta.env.DEV) {
  const { installRemoteConsole } = await import("@/feature/dev/remote-console.ts");
  installRemoteConsole();
}

try {
  const missing = await whatIsMissing();
  if (missing) bootstrapI18n();
  else bootstrapApp();

  const container = document.getElementById("root");
  if (!container) throw new Error("web: missing root container: #root");
  // Registered as a device's build registers it, which is also what gives
  // the tree a root that fills the page as a device's window.
  AppRegistry.registerComponent("pixelet", () => () => (
    <StrictMode>
      {missing ? <Unsupported reason={missing} /> : <App />}
    </StrictMode>
  ));
  // A device's root is a native tag; react-native-web's is the element it
  // renders into, which React Native's types have no word for.
  AppRegistry.runApplication("pixelet", { rootTag: container as unknown as RootTag, initialProps: {} });
} catch (err) {
  console.error("[web] failed to start:", err);
}
