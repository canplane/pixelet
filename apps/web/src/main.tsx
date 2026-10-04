// @pxl/web/main.tsx

/**
 * The page's entry. An unsupported browser is shown why and nothing starts.
 * Failures are logged, not thrown, so the page never goes blank silently.
 */

import { StrictMode } from "react";
import { AppRegistry, type RootTag } from "react-native";

import { AppRouter } from "@/app/router.tsx";
import { bootstrapApp } from "@/app/bootstrap.ts";
import { bootstrapI18n } from "@/locale/i18n.ts";
import { Unsupported } from "@/platform/web/unsupported.tsx";
import { whatIsMissing } from "@/platform/web/webgpu.ts";
import "@/app/style/index.css";

if (import.meta.env.DEV) {
  const { installRemoteConsole } = await import("@/platform/web/remote-console.ts");
  installRemoteConsole();
}

try {
  const missing = await whatIsMissing();
  if (missing) bootstrapI18n();
  else bootstrapApp();

  const container = document.getElementById("root");
  if (!container) throw new Error("web: missing root container: #root");
  AppRegistry.registerComponent("pixelet", () => () => (
    <StrictMode>
      {missing ? <Unsupported reason={missing} /> : <AppRouter />}
    </StrictMode>
  ));
  AppRegistry.runApplication("pixelet", { rootTag: container as unknown as RootTag, initialProps: {} });
} catch (err) {
  console.error("[web] failed to start:", err);
}
