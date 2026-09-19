// @pxl/web/app/layout.tsx

import type { ReactNode } from "react";

import { AppContext } from "./provider/context.ts";

import { useAppInit } from "./hook/use-init.ts";
import { useUiController } from "./hook/use-ui-controller.ts";
import { ModalManager } from "./provider/modal-manager.tsx";
import { Toaster } from "@/shared/ui/toast/toaster.tsx";
import { BOTTOM_OFFSET_PX } from "@/feature/map/ui/overlay/bottom-overlays.tsx";
import { MODE_BAR_HEIGHT_PX } from "@/feature/map/ui/overlay/mode-bar.tsx";

// Toaster itself has no feature-specific knowledge — this is the composition
// root, so it's the one place allowed to know the map's mode bar exists and
// hand Toaster the room to stay clear of it.
const TOASTER_BOTTOM_PX = BOTTOM_OFFSET_PX + MODE_BAR_HEIGHT_PX;

/** Loading page */
function LoadingPage() {
  return null;
}

/** Error page */
function ErrorPage() {
  return null;
}

/** AppLayout props */
type AppLayoutProps = {
  children: ReactNode;
};

/** Root layout component */
export function AppLayout(props: AppLayoutProps) {
  const { children } = props;

  const { loadState, store } = useAppInit(); // init

  const { uiController, mergeUiController } = useUiController(store);

  return (
    <AppContext.Provider value={{ store, ui: uiController }}>
      <Toaster bottomPx={TOASTER_BOTTOM_PX} />

      <div className="min-h-screen flex flex-col items-center">
        {loadState === "success" && children}

        {loadState == "loading" && <LoadingPage />}
        {(loadState === "timeout" || loadState === "error") && <ErrorPage />}
      </div>

      {/* Global modal host */}
      <ModalManager mergeUiController={mergeUiController} />
    </AppContext.Provider>
  );
}
