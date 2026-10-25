// @pxl/web/main.tsx

/**
 * The page's entry. An unsupported browser is shown why and nothing starts.
 * Failures are logged, not thrown, so the page never goes blank silently.
 */

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import * as bracket from "@pxl/client/bracket.tsx";
import * as locale from "@pxl/client/shell/locale/i18n.ts";
import { boot } from "@pxl/client/shell/boot.ts";
import { AppRouter } from "@pxl/client/shell/router.tsx";
import { Halt } from "@pxl/client/shell/halt.tsx";
import { page } from "@/bracket.ts";
import { whatIsMissing } from "@/webgpu.ts";
import "@pxl/client/shell/style/index.css";

if (import.meta.env.DEV) (await import("@/remote-console.ts")).install();

bracket.set(page);

try {
  const missing = await whatIsMissing();
  if (missing) locale.boot();
  else boot();

  const root = document.getElementById("root");
  if (!root) throw new Error("web: missing root container: #root");
  createRoot(root).render(
    <StrictMode>
      {missing ? <Halt of={`halt.${missing}`} /> : <AppRouter />}
    </StrictMode>,
  );
} catch (err) {
  console.error("[web] failed to start:", err);
}
