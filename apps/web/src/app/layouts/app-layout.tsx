import type { ReactNode } from 'react';

import { AppContext } from '@/app/providers/app-context';

import { useAppInit } from '@/app/hooks/use-app-init';
import { useUiController } from '@/app/hooks/use-ui-controller';
import { ModalManager } from '@/app/providers/modal-manager';

/** Loading page */
function LoadingPage() {
  return <></>;
}

/** Error page */
function ErrorPage() {
  return <></>;
}

/** AppLayoutProps */
type AppLayoutProps = {
  children: ReactNode;
};

/** Root layout component */
export function AppLayout(props: AppLayoutProps) {
  const { children } = props;

  const { loadState, store } = useAppInit(); // init

  const { uiController, mergeUiController } = useUiController(store);

  return (
    <AppContext.Provider value={{ store, ui: uiController }}>
      <div className="min-h-screen flex flex-col items-center">
        {loadState === 'success' && children}

        {loadState == 'loading' && <LoadingPage />}
        {(loadState === 'timeout' || loadState === 'error') && <ErrorPage />}
      </div>

      {/* Global modal host */}
      <ModalManager mergeUiController={mergeUiController} />
    </AppContext.Provider>
  );
}
