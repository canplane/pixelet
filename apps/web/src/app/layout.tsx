// @pxl/web/app/layout.tsx

import { type ReactNode, useEffect } from "react";
import { StyleSheet, View } from "react-native";

import { Message } from "@/kernel/abi/cmd.spec.ts";
import { Gpu, useGpu } from "@/kernel/gpu.ts";
import { useKernel } from "@/kernel/shell.ts";
import { Boot, useBoot, useStopCode } from "./bootstrap.ts";
import { Modals } from "./modal.tsx";
import { Halt, KernelPanic } from "./halt.tsx";
import { messageKey } from "@/locale/message-key.ts";
import { KeyboardAway } from "@/shared/ui/keyboard.tsx";
import { DockMode, useUiStore } from "@/app/store/ui.ts";

/** What ends the app, said in place of it: no GPU it will ask for again, a kernel that never started, or reads given up. */
function haltOf(gpu: Gpu, boot: Boot, givenUp: boolean): string | null {
  if (gpu === Gpu.Missing) return "halt.gpu_failed";
  if (boot === Boot.Failed) return "halt.boot";
  return givenUp ? messageKey(Message.WorldUnavailable) : null;
}

export function AppLayout({ children }: { children: ReactNode }) {
  const gpu = useGpu();
  const boot = useBoot();
  const givenUp = useKernel((s) => s.stall === Message.WorldUnavailable);
  const stopCode = useStopCode();
  useEffect(() => {
    if (givenUp) console.error("[read] a read failed past every retry; the space is given up");
  }, [givenUp]);
  if (boot === Boot.Stopped) return <KernelPanic code={stopCode} />;
  const halt = haltOf(gpu, boot, givenUp);
  if (halt !== null) return <Halt of={halt} />;
  return (
    // A touch elsewhere puts the typing away with the keyboard.
    <KeyboardAway onAway={() => useUiStore.getState().setDockMode(DockMode.Palette)}>
      <View style={styles.page}>{boot === Boot.Started && children}</View>
      <Modals />
    </KeyboardAway>
  );
}

const styles = StyleSheet.create({
  page: { flex: 1, alignItems: "center" },
});
