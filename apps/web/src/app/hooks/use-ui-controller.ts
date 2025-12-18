import { useState, useCallback, useMemo } from 'react';

import type { UiController } from '../types/ui.types';

/**
 * Hook for managing the global UI controller.
 * Provides a stable imperative interface to trigger UI actions from anywhere.
 */
export function useUiController(store: unknown) {
  // Pure data store for registered actions
  const [actions, setActions] = useState<Partial<UiController>>({});

  /**
   * Progressively registers new UI actions into the controller.
   * Uses useCallback to maintain referential stability for child effects.
   */
  const mergeUiController = useCallback((newActions: Partial<UiController>) => {
    setActions((prev) => ({ ...prev, ...newActions }));
  }, []);

  /**
   * The actual controller interface exposed to the app.
   * Wrapped in a Proxy to prevent "is not a function" errors before registration.
   */
  const uiController = useMemo(() => {
    return new Proxy(actions as UiController, {
      get: (target, prop) => {
        const action = target[prop as keyof UiController];

        if (action) return action;

        // Fallback for unregistered actions
        return (...args: unknown[]) => {
          console.warn(
            `[UiController] Action "${String(prop)}" called but not registered.`,
            { args },
          );
        };
      },
    });
  }, [actions]);

  return { uiController, mergeUiController };
}
