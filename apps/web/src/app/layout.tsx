// @pxl/web/app/layout.tsx

import type { ReactNode } from "react";
import { StyleSheet, View } from "react-native";

import { AppContext } from "./provider/context.ts";

import { useBooted } from "./bootstrap.ts";
import { useUiController } from "./hook/use-ui-controller.ts";
import { ModalManager } from "./provider/modal-manager.tsx";
import { KeyboardAway } from "@/shared/ui/keyboard.tsx";

interface AppLayoutProps {
  children: ReactNode;
}

export function AppLayout(props: AppLayoutProps) {
  const { children } = props;

  const booted = useBooted();

  const { uiController, mergeUiController } = useUiController();

  return (
    <AppContext.Provider value={{ ui: uiController }}>
      <KeyboardAway>
        <View style={styles.page}>{booted && children}</View>

        <ModalManager mergeUiController={mergeUiController} />
      </KeyboardAway>
    </AppContext.Provider>
  );
}

const styles = StyleSheet.create({
  page: { flex: 1, alignItems: "center" },
});
