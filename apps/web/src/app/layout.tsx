// @pxl/web/app/layout.tsx

import type { ReactNode } from "react";
import { StyleSheet, View } from "react-native";

import { AppContext } from "./provider/context.ts";

import { useAppInit } from "./hook/use-init.ts";
import { useUiController } from "./hook/use-ui-controller.ts";
import { ModalManager } from "./provider/modal-manager.tsx";
import { UnsupportedNotice } from "./unsupported.tsx";

/** Loading page */
function LoadingPage() {
  return null;
}

/** Error page */
function ErrorPage() {
  return null;
}

/** AppLayout props */
interface AppLayoutProps {
  children: ReactNode;
}

/** Root layout component */
export function AppLayout(props: AppLayoutProps) {
  const { children } = props;

  const { loadState, store } = useAppInit(); // init

  const { uiController, mergeUiController } = useUiController(store);

  return (
    <AppContext.Provider value={{ store, ui: uiController }}>
      <View style={styles.page}>
        {loadState === "success" && children}

        {loadState == "loading" && <LoadingPage />}
        {(loadState === "timeout" || loadState === "error") && <ErrorPage />}
      </View>

      {/* Global modal host */}
      <ModalManager mergeUiController={mergeUiController} />

      {
        /* Last, and over all of it: nothing below this is worth reading once
          it renders. */
      }
      <UnsupportedNotice />
    </AppContext.Provider>
  );
}

const styles = StyleSheet.create({
  page: { flex: 1, alignItems: "center" },
});
