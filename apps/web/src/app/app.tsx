// @pxl/web/app/app.tsx

import { AppRouter } from "./router/app-router.tsx";

import { DevPanel } from "./dev/dev-panel.tsx";

/** Entry point of the application */
export function App() {
  return (
    <>
      <AppRouter />

      {/* Debugging UI */}
      {__IS_DEV__ && <DevPanel />}
    </>
  );
}
