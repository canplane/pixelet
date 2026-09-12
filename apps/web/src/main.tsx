// @pxl/web/main.tsx

import { enableMapSet } from "immer";
import { bootstrap_app } from "@/bootstrap/app.ts";

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "@/app/style/index.css";
import { App } from "@/app/app.tsx";

enableMapSet(); // enables Immer's Map/Set support plugin

let root: ReturnType<typeof createRoot> | null = null;

// starts rendering after async engine init
export function main(): void {
  try {
    // 1. infra bootstrap (Three.js basis, i18n)
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
