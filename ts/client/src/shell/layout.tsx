// @pxl/client/shell/layout.tsx

import { Component, type ReactNode } from "react";
import { StyleSheet, View } from "react-native";

import { BOOT_LOADING, BOOT_STARTED } from "@/abi/state.ts";
import { useStanding } from "./boot.ts";
import { coverOf, SHELL_PANIC } from "./cover.ts";
import { Modals } from "./modal.tsx";
import { Screen } from "./screens.tsx";
import { KeyboardAway } from "@/shell/ui/keyboard.tsx";
import { DockMode, setDockMode } from "@/shell/world/dock/store.ts";

/** The app under its cover. The world mounts once the kernel runs, and goes for a
 * panic, as nothing can be drawn or read behind one. */
function AppBody({ children }: { children: ReactNode }) {
  const standing = useStanding();
  const cover = coverOf(standing);
  const shows =
    (standing.boot === BOOT_LOADING || standing.boot === BOOT_STARTED) &&
    cover?.kind !== "panic";
  return (
    // A touch elsewhere puts the typing away with the keyboard.
    <KeyboardAway onAway={() => setDockMode(DockMode.Palette)}>
      <View style={styles.page}>{shows && children}</View>
      {shows && <Modals />}
      {cover !== null && (
        <View style={StyleSheet.absoluteFill}>
          <Screen cover={cover} />
        </View>
      )}
    </KeyboardAway>
  );
}

/** A throw while the shell renders is a panic, not a blank page. */
class ErrorBoundary extends Component<
  { children: ReactNode },
  { thrown: boolean }
> {
  override state = { thrown: false };

  static getDerivedStateFromError() {
    return { thrown: true };
  }

  override componentDidCatch(err: unknown) {
    console.error("[shell] a render threw:", err);
  }

  override render() {
    return this.state.thrown ? (
      <Screen cover={SHELL_PANIC} />
    ) : (
      this.props.children
    );
  }
}

export function AppLayout({ children }: { children: ReactNode }) {
  return (
    <ErrorBoundary>
      <AppBody>{children}</AppBody>
    </ErrorBoundary>
  );
}

const styles = StyleSheet.create({
  page: { flex: 1, alignItems: "center" },
});
