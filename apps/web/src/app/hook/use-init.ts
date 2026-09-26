// @pxl/web/app/hook/use-init.ts

import { useState } from "react";

import { useBootState } from "@/app/bootstrap.ts";

export const enum LoadState {
  IDLE = "idle", // Initial state
  LOADING = "loading", // Show loading UI (after threshold)
  SUCCESS = "success", // Initialization complete
  ERROR = "error", // Initialization failed
  TIMEOUT = "timeout", // Operation exceeded threshold
}

interface AppInitResult {
  loadState: LoadState;
  store: unknown;
}

/**
 * Hook to manage the global application initialization process: the host's
 * boot (app/bootstrap.ts).
 */
export function useAppInit(): AppInitResult {
  const boot = useBootState();
  const [store] = useState<unknown>({});
  const loadState = boot === "ready" ? LoadState.SUCCESS : boot === "failed" ? LoadState.ERROR : LoadState.LOADING;

  return { store, loadState };
}
