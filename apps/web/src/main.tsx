// @pxl/web/main.tsx

/**
 * The page's entry: index.html loads this and nothing else.
 *
 * Two steps in order — what the app needs before anything renders (the
 * locale, the kernel's boot), then the tree itself. A failure in either is
 * logged rather than thrown: the page is already on screen by then, and an
 * uncaught error here would leave it blank with nothing said.
 */

import { StrictMode } from "react";
import { AppRegistry, type RootTag } from "react-native";

import { App } from "@/app/app.tsx";
import { bootstrapApp } from "@/app/bootstrap.ts";
import "@/app/style/index.css";

// Before anything else, so a failure in the boot below is itself forwarded.
if (import.meta.env.DEV) {
  const { installRemoteConsole } = await import("@/feature/dev/remote-console.ts");
  installRemoteConsole();
}

try {
  bootstrapApp();

  const container = document.getElementById("root");
  if (!container) throw new Error("web: missing root container: #root");
  // Registered as a device's build registers it, which is also what gives
  // the tree a root that fills the page as a device's window.
  AppRegistry.registerComponent("pixelet", () => () => (
    <StrictMode>
      <App />
    </StrictMode>
  ));
  // A device's root is a native tag; react-native-web's is the element it
  // renders into, which React Native's types have no word for.
  AppRegistry.runApplication("pixelet", { rootTag: container as unknown as RootTag, initialProps: {} });
} catch (err) {
  console.error("[web] failed to start:", err);
}
