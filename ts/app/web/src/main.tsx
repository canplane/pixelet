// @pxl/web/main.tsx

/**
 * The page's entry. An unsupported browser is shown why and nothing starts.
 * Failures are logged, not thrown, so the page never goes blank silently.
 */

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import * as bracket from "@pxl/client/bracket.tsx";
import * as locale from "@pxl/client/shell/locale/i18n.ts";
import { boot } from "@pxl/client/abi/boot.ts";
import { AppLayout } from "@pxl/client/shell/layout.tsx";
import WorldPage from "@pxl/client/shell/world/page.tsx";
import { Halt } from "@pxl/client/shell/halt.tsx";
import { page } from "./bracket.ts";
import * as webgpu from "./webgpu.ts";
import "./document.css";

if (import.meta.env.DEV) (await import("./remote-console.ts")).install();

bracket.set(page);

try {
  // The shell's words first: whatever it shows, a halt among it, is in them.
  locale.boot();
  const missing = await webgpu.missing();
  // The client, bottom up, where there is a GPU to boot it on; the shell
  // below reads where it stands.
  if (!missing) boot();

  const root = document.getElementById("root");
  if (!root) throw new Error("web: missing root container: #root");
  createRoot(root).render(
    <StrictMode>
      {missing ? (
        <Halt of={`halt.${missing}`} />
      ) : (
        <AppLayout>
          <WorldPage />
        </AppLayout>
      )}
    </StrictMode>,
  );
} catch (err) {
  console.error("[web] failed to start:", err);
}
