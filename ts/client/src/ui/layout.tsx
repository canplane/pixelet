// @pxl/client/ui/layout.tsx

import { Component, type ReactNode, useEffect } from "react";
import { StyleSheet, View } from "react-native";

import {
  BOOT_LOADING,
  BOOT_STARTED,
  BOOT_STARTING,
} from "@/service/sys/boot.ts";
import { installConsole } from "@/ui/feature/debug/console.ts";
import { useStanding } from "./feature/cover/standing.ts";
import { coverOf, UI_PANIC } from "./feature/cover/cover.ts";
import { ModalHost } from "./modal.tsx";
import { Screen } from "./feature/cover/screen.tsx";
import { KeyboardAway } from "@/ui/shared/component/keyboard.tsx";
import { DockMode, setDockMode } from "@/ui/feature/world/dock/store.ts";
import { MESSAGE_WORLD_UNAVAILABLE } from "@/abi/overlay.enum.ts";

/** The world mounts once it can be made, a GPU present and the source
 * declared (`BOOT_LOADING`), and goes at a panic, because nothing can be drawn
 * or read behind one. */
function AppBody({ children }: { children: ReactNode }) {
  const standing = useStanding();
  const cover = coverOf(standing);
  // A console over the core is installed as the world begins to be made.
  useEffect(() => {
    if (standing.boot === BOOT_LOADING) installConsole();
  }, [standing.boot]);
  useEffect(() => {
    if (standing.stall === MESSAGE_WORLD_UNAVAILABLE) {
      console.error(
        "[read] a read failed past every retry; the space is given up",
      );
    }
  }, [standing.stall]);
  // A build made for measuring measures from the core's start
  // (perf/measure.ts); any other build holds no way to.
  useEffect(() => {
    if (__PERF_DRIVE__ && standing.boot === BOOT_STARTING)
      void import("@/ui/feature/debug/perf/measure.ts").then((m) =>
        m.measure(),
      );
  }, [standing.boot]);
  const shows =
    (standing.boot === BOOT_LOADING || standing.boot === BOOT_STARTED) &&
    cover?.kind !== "panic";
  return (
    <KeyboardAway onAway={() => setDockMode(DockMode.PALETTE)}>
      <View style={STYLE.page}>{shows && children}</View>
      {shows && <ModalHost />}
      {cover !== null && (
        <View style={StyleSheet.absoluteFill}>
          <Screen cover={cover} />
        </View>
      )}
    </KeyboardAway>
  );
}

/** A throw while the UI renders is a panic, not a blank page. */
class ErrorBoundary extends Component<
  { children: ReactNode },
  { thrown: boolean }
> {
  override state = { thrown: false };

  static getDerivedStateFromError() {
    return { thrown: true };
  }

  override componentDidCatch(err: unknown) {
    console.error("[ui] a render threw:", err);
  }

  override render() {
    return this.state.thrown ? (
      <Screen cover={UI_PANIC} />
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

const STYLE = StyleSheet.create({
  page: { flex: 1, alignItems: "center" },
});
