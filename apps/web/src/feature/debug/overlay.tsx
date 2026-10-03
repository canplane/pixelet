// @pxl/web/feature/debug/overlay.tsx

/**
 * The debug overlay: where the camera and the pointer are, and the profiler's
 * recording controls, exports and gauge. Shown by the `debug_overlay` setting
 * in any build; the kernel records from boot while it is on. Exposes `__pxl_profiler_trace`, `__pxl_profiler_log`,
 * `__pxl_view` and `__pxl_gauge` on `window` for A/B runs.
 */

import { CaretDown, CaretUp } from "phosphor-react-native";
import { useEffect, useState } from "react";
import { Pressable, ScrollView, StyleSheet, Text, View } from "react-native";

import { alpha, color, disabledContent } from "@/shared/ui/theme.ts";
import { gauge, ProfilerGauge, ProfilerRec, ProfilerView, Title } from "./profiler/gauge.tsx";
import { profilerLog } from "./profiler/log.ts";
import { profilerReportCopy, profilerReportText } from "./profiler/report.ts";
import { type Gauge, profilerGaugeRead, profilerReportRead } from "./profiler/view.ts";
import { installLongFrameObservers, longFramesClear } from "./profiler/long-frame.ts";
import { profilerTraceSave } from "./profiler/trace.ts";
import { Command, encode_input_view } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, useKernel } from "@/kernel/shell.ts";
import { SETTINGS } from "@/feature/setting/schema.ts";

const enum Tint {
  Plain = 0,
  Go = 1,
  Halt = 2,
  Off = 3,
}

const TINT: Record<Tint, { border: string; ground: string; ink: string; lit: string; litInk: string }> = {
  [Tint.Plain]: { border: alpha(color.white, 0.15), ground: alpha(color.white, 0.05), ink: alpha(color.white, 0.6), lit: alpha(color.white, 0.1), litInk: alpha(color.white, 0.9) },
  [Tint.Go]: { border: alpha(color.profilerGo, 0.4), ground: alpha(color.profilerGo, 0.1), ink: color.profilerGo, lit: alpha(color.profilerGo, 0.2), litInk: color.profilerGo },
  [Tint.Halt]: { border: alpha(color.profilerHalt, 0.4), ground: alpha(color.profilerHalt, 0.1), ink: color.profilerHalt, lit: alpha(color.profilerHalt, 0.2), litInk: color.profilerHalt },
  [Tint.Off]: { border: alpha(color.white, 0.1), ground: alpha(color.white, 0.05), ink: alpha(color.white, 0.25), lit: alpha(color.white, 0.05), litInk: alpha(color.white, 0.25) },
};

/** The hint shows under the buttons on hover or hold, since a device has no tooltip. */
function PanelButton({ label, hint, tint = Tint.Plain, onPress, onHint, disabled }: {
  label: string;
  hint: string;
  tint?: Tint;
  onPress: () => void;
  onHint: (hint: string | null) => void;
  disabled?: boolean;
}) {
  const [hovered, setHovered] = useState(false);
  const t = TINT[tint];
  return (
    <Pressable
      onPress={onPress}
      onHoverIn={() => {
        setHovered(true);
        onHint(hint);
      }}
      onHoverOut={() => {
        setHovered(false);
        onHint(null);
      }}
      onLongPress={() => onHint(hint)}
      onPressOut={() => {
        if (!hovered) onHint(null);
      }}
      disabled={disabled}
      accessibilityRole="button"
      accessibilityHint={hint}
      style={({ pressed }) => [styles.button, { borderColor: t.border, backgroundColor: hovered || pressed ? t.lit : t.ground }]}
    >
      {({ pressed }) => <Text style={[gauge.text, { color: hovered || pressed ? t.litInk : t.ink }, disabled && { opacity: disabledContent }]}>{label}</Text>}
    </Pressable>
  );
}

const POLL_MS = 500;

/** Folding every frame since load on each repaint grew into a 60 ms task. */
const LIVE_WINDOW_MS = 10_000;

const SAID_MS = 2500;

type ProfilerGlobals = typeof globalThis & {
  __pxl_profiler_trace?: () => Promise<string>;
  __pxl_profiler_log?: (on: boolean) => void;
  __pxl_view?: (dist: number | null, polar: number | null) => void;
  __pxl_gauge?: (windowMs: number) => Gauge;
};

function restartWindow(): void {
  kernel_command(Command.PROFILER_START);
  longFramesClear();
}

export function DebugOverlay() {
  const show = useKernel((s) => s.settings[SETTINGS.debug_overlay.key] === 1);
  const [hint, setHint] = useState<string | null>(null);
  const [said, setSaid] = useState<string | null>(null);
  const [g, setG] = useState<Gauge | null>(null);
  const [viewOpen, setViewOpen] = useState(true);
  const [open, setOpen] = useState(true);

  useEffect(() => {
    if (!show) return;
    // Folded, only the view is shown, so the window is left unfolded.
    const read = () => setG(profilerGaugeRead(open ? LIVE_WINDOW_MS : 0));
    read();
    const id = setInterval(read, POLL_MS);
    return () => clearInterval(id);
  }, [show, open]);

  useEffect(() => {
    if (said === null) return;
    const id = setTimeout(() => setSaid(null), SAID_MS);
    return () => clearTimeout(id);
  }, [said]);

  useEffect(() => {
    if (!show) return;
    // Cleared before the observers are installed: they replay what the page
    // buffered since load, so a panel on from boot has the boot's frames.
    longFramesClear();
    installLongFrameObservers();
    const w = globalThis as ProfilerGlobals;
    w.__pxl_profiler_trace = profilerTraceSave;
    w.__pxl_profiler_log = profilerLog;
    w.__pxl_view = (dist, polar) => kernel_command(Command.INPUT_VIEW, encode_input_view({ dist, polar }));
    w.__pxl_gauge = (windowMs) => profilerGaugeRead(windowMs);
    return () => {
      profilerLog(false);
      delete w.__pxl_profiler_trace;
      delete w.__pxl_profiler_log;
      delete w.__pxl_view;
      delete w.__pxl_gauge;
    };
  }, [show]);

  if (!show) return null;
  const recording = g?.rec.recording === true;

  return (
    <View style={styles.panel}>
      <Pressable onPress={() => setViewOpen(!viewOpen)} accessibilityRole="button" aria-expanded={viewOpen}>
        <Title first end={<Caret open={viewOpen} />}>view</Title>
      </Pressable>
      {viewOpen && g !== null && <ProfilerView v={g.view} />}

      <Pressable onPress={() => setOpen(!open)} accessibilityRole="button" aria-expanded={open}>
        <Title
          end={
            <View style={styles.titleEnd}>
              <Text style={[gauge.text, gauge.key]}>last {LIVE_WINDOW_MS / 1000} s</Text>
              <Caret open={open} />
            </View>
          }
        >
          performance
        </Title>
      </Pressable>
      {open && (
        <>
          <View style={styles.buttons}>
            {
              /* Both, not a toggle: a window opened at boot would read "stop"
                          until first stopped. */
            }
            <PanelButton
              onHint={setHint}
              label={recording ? "restart" : "start"}
              hint="clear everything and begin a fresh window"
              tint={recording ? Tint.Plain : Tint.Go}
              onPress={restartWindow}
            />
            <PanelButton
              onHint={setHint}
              label="stop"
              hint="freeze the window so exports are stable while you read them"
              tint={recording ? Tint.Halt : Tint.Off}
              disabled={!recording}
              onPress={() => kernel_command(Command.PROFILER_STOP)}
            />
            <PanelButton
              onHint={setHint}
              label="report"
              hint="copy the window's summary as text"
              onPress={() => void profilerReportCopy(profilerReportText(profilerReportRead())).then(setSaid)}
            />
            <PanelButton onHint={setHint} label="trace" hint="save the window as a timeline for ui.perfetto.dev" onPress={() => void profilerTraceSave().then(setSaid)} />
          </View>
          {
            /* Under the buttons: a two-line hint above them moved the hovered
                      button away. */
          }
          {(said ?? hint) !== null && <Text style={[gauge.text, styles.hint]}>{said ?? hint}</Text>}
          {g !== null && <ProfilerRec rec={g.rec} />}
          <ScrollView style={styles.body} showsVerticalScrollIndicator={false}>
            <ProfilerGauge g={g} />
          </ScrollView>
        </>
      )}
    </View>
  );
}

const HINT_INK = alpha(color.white, 0.55);

const Caret = ({ open }: { open: boolean }) => (open ? <CaretUp size={12} color={HINT_INK} /> : <CaretDown size={12} color={HINT_INK} />);

const styles = StyleSheet.create({
  panel: {
    flexShrink: 1,
    width: 300,
    overflow: "hidden",
    borderRadius: 8,
    padding: 10,
    backgroundColor: alpha(color.black, 0.72),
    pointerEvents: "box-none",
  },
  titleEnd: { flexDirection: "row", alignItems: "center", gap: 6 },
  hint: { marginTop: 6, fontWeight: "600", letterSpacing: 0.25, color: HINT_INK },
  body: { flexShrink: 1 },
  buttons: { flexDirection: "row", gap: 4, marginTop: 8 },
  button: { flex: 1, alignItems: "center", paddingVertical: 4, borderWidth: 1, borderRadius: 4 },
});
