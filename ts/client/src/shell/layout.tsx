// @pxl/client/shell/layout.tsx

import { type ReactNode, useEffect } from "react";
import { StyleSheet, View } from "react-native";

import { Message } from "@/abi/cmd.spec.ts";
import { Gpu } from "@/abi/gpu.ts";
import { Boot, useBoot, useStop } from "./boot.ts";
import { Modals } from "./modal.tsx";
import { BlueScreen, Halt } from "./halt.tsx";
import { messageKey } from "@/shell/locale/message-key.ts";
import { KeyboardAway } from "@/shell/ui/keyboard.tsx";
import { DockMode, useDockStore } from "@/shell/store/dock.ts";
import { useGpu, useKernel } from "@/shell/store/watch.ts";

/** What ends the app before it ran, said in place of it: no GPU to start with, or a kernel that never started. */
function haltOf(gpu: Gpu, boot: Boot): string | null {
  if (gpu === Gpu.Missing) return "halt.gpu_failed";
  return boot === Boot.Failed ? "halt.boot" : null;
}

/** What ended the app after it ran, as a blue screen: its words and its stop code — the kernel's
 * report, or the name of what was given up. */
function endedOf(gpu: Gpu, boot: Boot, givenUp: boolean, stopCode: string | null): [string, string | null] | null {
  if (boot === Boot.Stopped) return ["halt.kernel_panic", stopCode];
  if (gpu === Gpu.Lost) return ["halt.gpu_lost", "GPU_LOST"];
  return givenUp ? [messageKey(Message.WorldUnavailable), "WORLD_UNAVAILABLE"] : null;
}

export function AppLayout({ children }: { children: ReactNode }) {
  const gpu = useGpu();
  const boot = useBoot();
  const givenUp = useKernel((s) => s.stall === Message.WorldUnavailable);
  const stopCode = useStop();
  useEffect(() => {
    if (givenUp) console.error("[read] a read failed past every retry; the space is given up");
  }, [givenUp]);
  const ended = endedOf(gpu, boot, givenUp, stopCode);
  if (ended !== null) return <BlueScreen of={ended[0]} code={ended[1]} />;
  const halt = haltOf(gpu, boot);
  if (halt !== null) return <Halt of={halt} />;
  return (
    // A touch elsewhere puts the typing away with the keyboard.
    <KeyboardAway onAway={() => useDockStore.getState().setDockMode(DockMode.Palette)}>
      <View style={styles.page}>{boot === Boot.Started && children}</View>
      <Modals />
    </KeyboardAway>
  );
}

const styles = StyleSheet.create({
  page: { flex: 1, alignItems: "center" },
});
