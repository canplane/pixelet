// @pxl/web/app/layout.tsx

import type { ReactNode } from "react";
import { StyleSheet, View } from "react-native";

import { useBooted } from "./bootstrap.ts";
import { Modals } from "./modal.tsx";
import { KeyboardAway } from "@/shared/ui/keyboard.tsx";

export function AppLayout({ children }: { children: ReactNode }) {
  const booted = useBooted();
  return (
    <KeyboardAway>
      <View style={styles.page}>{booted && children}</View>
      <Modals />
    </KeyboardAway>
  );
}

const styles = StyleSheet.create({
  page: { flex: 1, alignItems: "center" },
});
