// @pxl/web/main.tsx

// import "@pxl/common/lib/i18n/i18n.client.ts"; // i18n initialization

import { enableMapSet } from "immer";
import { bootstrap_app } from "@/bootstrap/app.ts";

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "@/app/styles/index.css";
import { App } from "@/app/app.tsx";

enableMapSet(); // Immer의 Map/Set 지원 플러그인 활성화

let root: ReturnType<typeof createRoot> | null = null;

export async function main(): Promise<void> {
  try {
    await bootstrap_app();

    if (!root) {
      const container = document.getElementById("root");
      if (!container) throw new Error("web: missing root container: #root");
      root = createRoot(container);
    }

    root.render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
  } catch (err) {
    console.error("web: failed to bootstrap app:", err);
  }
}
// main();
