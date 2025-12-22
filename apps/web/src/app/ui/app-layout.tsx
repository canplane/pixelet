import type { ReactNode } from 'react';

import { AppContext } from '../providers/app-context';

import { useAppInit } from '../hooks/use-app-init';
import { useUiController } from '../hooks/use-ui-controller';
import { ModalManager } from '../providers/modal-manager';
import { Toaster } from '@/shared/ui/theme/glass/sonner';

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
      <Toaster />

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
