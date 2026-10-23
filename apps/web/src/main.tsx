// @pxl/web/main.tsx

/**
 * The page's entry. An unsupported browser is shown why and nothing starts.
 * Failures are logged, not thrown, so the page never goes blank silently.
 */

import { StrictMode } from "react";
import { AppRegistry, type RootTag } from "react-native";

import { setBracket } from "@pxl/client/bracket.tsx";
import { AppRouter } from "@pxl/client/shell/router.tsx";
import { bootstrapApp } from "@pxl/client/shell/bootstrap.ts";
import { bootstrapI18n } from "@pxl/client/shell/locale/i18n.ts";
import { Halt } from "@pxl/client/shell/halt.tsx";
import { PAGE } from "@/bracket.ts";
import { whatIsMissing } from "@/webgpu.ts";
import "@pxl/client/shell/style/index.css";

if (import.meta.env.DEV) {
  const { installRemoteConsole } = await import("@/remote-console.ts");
  installRemoteConsole();
}

setBracket(PAGE);

try {
  const missing = await whatIsMissing();
  if (missing) bootstrapI18n();
  else bootstrapApp();

  const container = document.getElementById("root");
  if (!container) throw new Error("web: missing root container: #root");
  AppRegistry.registerComponent("pixelet", () => () => (
    <StrictMode>
      {missing ? <Halt of={`halt.${missing}`} /> : <AppRouter />}
    </StrictMode>
  ));
  AppRegistry.runApplication("pixelet", { rootTag: container as unknown as RootTag, initialProps: {} });
} catch (err) {
  console.error("[web] failed to start:", err);
}
