// @pxl/web/app/provider/context.ts

import { createContext, useContext } from "react";

import type { UiController } from "../type/ui.type.ts";

/**
 * Values provided by the AppContext
 */
interface AppContextValue {
  /**
   * Imperative UI controller
   *
   * Provides access to global UI actions
   * such as opening modals or triggering overlays.
   */
  ui: UiController;
}

/**
 * Application-level context
 *
 * This context provides access to the imperative UI controls (ui
 * controller).
 *
 * It should be mounted once at the root of the app.
 */
export const AppContext = createContext<AppContextValue | null>(null);

/**
 * Hook for consuming the AppContext
 *
 * Throws an error if used outside of the AppContext provider,
 * enforcing correct usage at runtime.
 */
export function useAppContext(): AppContextValue {
  const context = useContext(AppContext);

  if (!context) {
    throw new Error("useAppContext must be used within AppProvider");
  }

  return context;
}
