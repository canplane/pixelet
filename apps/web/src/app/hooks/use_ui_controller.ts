// @pxl/web/app/hooks/use_ui_controller.ts

import { useCallback, useMemo, useState } from "react";

import type { UIController } from "../types/ui.types.ts";

/**
 * Hook for managing the global UI controller.
 * Provides a stable imperative interface to trigger UI actions from anywhere.
 */
export function useUIController(store: unknown) {
  // Pure data store for registered actions
  const [actions, set_actions] = useState<Partial<UIController>>({});

  /**
   * Progressively registers new UI actions into the controller.
   * Uses useCallback to maintain referential stability for child effects.
   */
  const merge_ui_controller = useCallback((new_actions: Partial<UIController>) => {
    set_actions((prev) => ({ ...prev, ...new_actions }));
  }, []);

  /**
   * The actual controller interface exposed to the app.
   * Wrapped in a Proxy to prevent "is not a function" errors before registration.
   */
  const ui_controller = useMemo(() => {
    return new Proxy(actions as UIController, {
      get: (target, prop) => {
        const action = target[prop as keyof UIController];

        if (action) return action;

        // Fallback for unregistered actions
        return (...args: unknown[]) => {
          console.warn(`[UIController] Action "${String(prop)}" called but not registered.`, { args });
        };
      },
    });
  }, [actions]);

  return { ui_controller, merge_ui_controller };
}
