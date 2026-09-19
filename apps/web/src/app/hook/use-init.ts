// @pxl/web/app/hook/use-init.ts

import { useState } from "react";

export const enum LoadState {
  IDLE = "idle", // Initial state
  LOADING = "loading", // Show loading UI (after threshold)
  SUCCESS = "success", // Initialization complete
  ERROR = "error", // Initialization failed
  TIMEOUT = "timeout", // Operation exceeded threshold
}

type AppInitResult = {
  loadState: LoadState;
  store: unknown;
};

/**
 * Hook to manage the global application initialization process.
 */
export function useAppInit(): AppInitResult {
  const [loadState] = useState<LoadState>(LoadState.SUCCESS);
  const [store] = useState<unknown>({});

  return { store, loadState };
}
