import { AppRouter } from './router/app-router';

import { DevPanel } from './dev/dev-panel';

const IS_DEV = import.meta.env.DEV;

/** Entry point of the application */
export function App() {
  return (
    <>
      <AppRouter />

      {/* Debugging UI */}
      {IS_DEV && <DevPanel />}
    </>
  );
}
