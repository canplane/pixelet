// @pxl/web/main.tsx

import { bootstrap_app } from "@/app/bootstrap.ts";

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "@/app/style/index.css";
import { App } from "@/app/app.tsx";

let root: ReturnType<typeof createRoot> | null = null;

// starts rendering after async engine init
export function main(): void {
  try {
    // 1. infra bootstrap (i18n, the settings)
    bootstrap_app();

    // 2. UI render
    if (!root) {
      const container = document.getElementById("root");
      if (!container) throw new Error(`web: missing root container: #root`);
      root = createRoot(container);
    }

    root.render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
  } catch (err) {
    console.error("[web] failed to bootstrap engines:", err);
    // TODO: show a fallback screen on engine load failure, if needed
  }
}
// main();
