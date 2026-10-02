// @pxl/web/app/provider/context.ts

import { createContext, useContext } from "react";

import type { UiController } from "../type/ui.type.ts";

interface AppContextValue {
  ui: UiController;
}

export const AppContext = createContext<AppContextValue | null>(null);

export function useAppContext(): AppContextValue {
  const context = useContext(AppContext);

  if (!context) {
    throw new Error("useAppContext must be used within AppProvider");
  }

  return context;
}
