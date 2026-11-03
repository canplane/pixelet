// @pxl/client/shell/layout.tsx

import { type ReactNode, useEffect } from "react";
import { StyleSheet, View } from "react-native";

import { Message } from "@/abi/cmd.spec.ts";
import { Boot } from "@/abi/state.ts";
import { useBoot, useStop } from "./boot.ts";
import { Modals } from "./modal.tsx";
import { type Cover, coverOf } from "./cover.ts";
import { Failed, Panic, Progress } from "./screens.tsx";
import { KeyboardAway } from "@/shell/ui/keyboard.tsx";
import { DockMode, setDockMode } from "@/shell/world/dock/store.ts";
import {
  useGpu,
  useKernel,
  useOnline,
  useSource,
} from "@/shell/store/watch.ts";

/** Failed or progress: over the world being made, or alone before it. */
function CoverOf({ cover }: { cover: Exclude<Cover, { kind: "panic" }> }) {
  return cover.kind === "progress" ? (
    <Progress of={cover.of} />
  ) : (
    <Failed of={cover.of} />
  );
}

export function AppLayout({ children }: { children: ReactNode }) {
  const gpu = useGpu();
  const boot = useBoot();
  const source = useSource();
  const online = useOnline();
  const givenUp = useKernel((s) => s.stall === Message.WorldUnavailable);
  const stopCode = useStop();
  const made = useKernel((s) => s.renderer_progress);
  useEffect(() => {
    if (givenUp) {
      console.error(
        "[read] a read failed past every retry; the space is given up",
      );
    }
  }, [givenUp]);
  const readFailing = useKernel((s) => s.stall === Message.WorldReadFailing);
  const cover = coverOf({
    boot,
    gpu,
    source,
    online,
    givenUp,
    readFailing,
    stopCode,
    made,
  });
  if (cover?.kind === "panic") {
    return <Panic of={cover.of} code={cover.code} />;
  }
  const runs = boot === Boot.Loading || boot === Boot.Started;
  if (!runs && cover !== null) return <CoverOf cover={cover} />;
  return (
    // A touch elsewhere puts the typing away with the keyboard.
    <KeyboardAway onAway={() => setDockMode(DockMode.Palette)}>
      <View style={styles.page}>{runs && children}</View>
      <Modals />
      {cover !== null && (
        <View style={StyleSheet.absoluteFill}>
          <CoverOf cover={cover} />
        </View>
      )}
    </KeyboardAway>
  );
}

const styles = StyleSheet.create({
  page: { flex: 1, alignItems: "center" },
});
