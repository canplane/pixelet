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
import { createRoot } from "react-dom/client";

import { App } from "@/app/app.tsx";
import { bootstrap_app } from "@/app/bootstrap.ts";
import "@/app/style/index.css";

try {
  bootstrap_app();

  const container = document.getElementById("root");
  if (!container) throw new Error("web: missing root container: #root");
  createRoot(container).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
} catch (err) {
  console.error("[web] failed to start:", err);
}
