// @pxl/web/app/app.tsx

import { AppRouter } from "./router/app_router.tsx";

import { DevPanel } from "./dev/dev_panel.tsx";

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
