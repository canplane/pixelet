// @pxl/web/app/ui/app_layout.tsx

import type { ReactNode } from "react";

import { AppContext } from "../providers/app_context.ts";

import { useAppInit } from "../hooks/use_app_init.ts";
import { useUIController } from "../hooks/use_ui_controller.ts";
import { ModalManager } from "../providers/modal_manager.tsx";
import { Toaster } from "@/shared/ui/theme/glass/sonner.tsx";

/** Loading page */
function LoadingPage() {
  return null;
}

/** Error page */
function ErrorPage() {
  return null;
}

/** AppLayoutProps */
type AppLayoutProps = {
  children: ReactNode;
};

/** Root layout component */
export function AppLayout(props: AppLayoutProps) {
  const { children } = props;

  const { load_state, store } = useAppInit(); // init

  const { ui_controller, merge_ui_controller } = useUIController(store);

  return (
    <AppContext.Provider value={{ store, ui: ui_controller }}>
      <Toaster />

      <div className="min-h-screen flex flex-col items-center">
        {load_state === "success" && children}

        {load_state == "loading" && <LoadingPage />}
        {(load_state === "timeout" || load_state === "error") && <ErrorPage />}
      </div>

      {/* Global modal host */}
      <ModalManager merge_ui_controller={merge_ui_controller} />
    </AppContext.Provider>
  );
}
