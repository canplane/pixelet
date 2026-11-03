// @pxl/web/main.tsx

/**
 * The page's entry: what the device's is (ts/app/native/index.ts), on the page.
 */

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import * as bracket from "@pxl/client/bracket.tsx";
import * as locale from "@pxl/client/shell/locale/i18n.ts";
import { boot } from "@pxl/client/abi/boot.ts";
import { AppLayout } from "@pxl/client/shell/layout.tsx";
import WorldPage from "@pxl/client/shell/world/page.tsx";
import { page } from "./bracket.ts";
import "./document.css";

if (import.meta.env.DEV) (await import("./remote-console.ts")).install();

bracket.set(page);

// The shell's words first, then the client, bottom up; the shell reads where it stands.
locale.boot();
boot();

const root = document.getElementById("root");
if (!root) throw new Error("web: missing root container: #root");
createRoot(root).render(
  <StrictMode>
    <AppLayout>
      <WorldPage />
    </AppLayout>
  </StrictMode>,
);
