// @pxl/web/app/hook/use-ui-controller.ts

import { useCallback, useMemo, useState } from "react";

import type { UiController } from "../type/ui.type.ts";

export function useUiController() {
  const [actions, setActions] = useState<Partial<UiController>>({});

  const mergeUiController = useCallback((newActions: Partial<UiController>) => {
    setActions((prev) => ({ ...prev, ...newActions }));
  }, []);

  /** A Proxy, so calling an action before it is registered is a no-op, not a TypeError. */
  const uiController = useMemo(() => {
    return new Proxy(actions as UiController, {
      get: (target, prop) => {
        const action = target[prop as keyof UiController];

        if (action) return action;

        return (...args: unknown[]) => {
          console.warn(`[uiController] action "${String(prop)}" called but not registered.`, { args });
        };
      },
    });
  }, [actions]);

  return { uiController, mergeUiController };
}
