// @pxl/web/app/ui/app-layout.tsx

import type { ReactNode } from "react";

import { AppContext } from "../providers/app-context.ts";

import { useAppInit } from "../hooks/use-app-init.ts";
import { useUIController } from "../hooks/use-ui-controller.ts";
import { ModalManager } from "../providers/modal-manager.tsx";
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

  const { loadState, store } = useAppInit(); // init

  const { uiController, mergeUIController } = useUIController(store);

  return (
    <AppContext.Provider value={{ store, ui: uiController }}>
      <Toaster />

      <div className="min-h-screen flex flex-col items-center">
        {loadState === "success" && children}

        {loadState == "loading" && <LoadingPage />}
        {(loadState === "timeout" || loadState === "error") && <ErrorPage />}
      </div>

      {/* Global modal host */}
      <ModalManager mergeUIController={mergeUIController} />
    </AppContext.Provider>
  );
}
