// @pxl/web/app/layout.tsx

import type { ReactNode } from "react";
import { StyleSheet, View } from "react-native";

import { AppContext } from "./provider/context.ts";

import { useBooted } from "./bootstrap.ts";
import { useUiController } from "./hook/use-ui-controller.ts";
import { ModalManager } from "./provider/modal-manager.tsx";
import { UnsupportedNotice } from "./unsupported.tsx";
import { KeyboardAway } from "@/shared/ui/keyboard.tsx";

/** AppLayout props */
interface AppLayoutProps {
  children: ReactNode;
}

/** Root layout component */
export function AppLayout(props: AppLayoutProps) {
  const { children } = props;

  const booted = useBooted();

  const { uiController, mergeUiController } = useUiController();

  return (
    <AppContext.Provider value={{ ui: uiController }}>
      <KeyboardAway>
        <View style={styles.page}>{booted && children}</View>

        {/* Global modal host */}
        <ModalManager mergeUiController={mergeUiController} />
      </KeyboardAway>

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
