// @pxl/web/app/layout.tsx

import type { ReactNode } from "react";
import { StyleSheet, View } from "react-native";

import { BootFailed } from "./boot-failed.tsx";
import { Boot, useBoot } from "./bootstrap.ts";
import { Modals } from "./modal.tsx";
import { KeyboardAway } from "@/shared/ui/keyboard.tsx";
import { DockMode, useUiStore } from "@/app/store/ui.ts";

export function AppLayout({ children }: { children: ReactNode }) {
  const boot = useBoot();
  return (
    // A touch elsewhere puts the typing away with the keyboard.
    <KeyboardAway onAway={() => useUiStore.getState().setDockMode(DockMode.Palette)}>
      <View style={styles.page}>{boot === Boot.Started && children}</View>
      {boot === Boot.Failed && <BootFailed />}
      <Modals />
    </KeyboardAway>
  );
}

const styles = StyleSheet.create({
  page: { flex: 1, alignItems: "center" },
});
