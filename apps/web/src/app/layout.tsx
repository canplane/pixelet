// @pxl/web/app/layout.tsx

import type { ReactNode } from "react";
import { StyleSheet, View } from "react-native";

import { useBooted } from "./bootstrap.ts";
import { LoadCover } from "./load-cover.tsx";
import { Modals } from "./modal.tsx";
import { KeyboardAway } from "@/shared/ui/keyboard.tsx";
import { DockMode, useUiStore } from "@/app/store/ui.ts";

export function AppLayout({ children }: { children: ReactNode }) {
  const booted = useBooted();
  return (
    // A touch elsewhere puts the typing away with the keyboard.
    <KeyboardAway onAway={() => useUiStore.getState().setDockMode(DockMode.Palette)}>
      <View style={styles.page}>{booted && children}</View>
      <LoadCover />
      <Modals />
    </KeyboardAway>
  );
}

const styles = StyleSheet.create({
  page: { flex: 1, alignItems: "center" },
});
