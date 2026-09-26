// @pxl/web/feature/dev/ledger/overlay.tsx

/**
 * Dev overlay for the kernel's ledger (crates/pxl/kernel/src/sched/ledger):
 * the recording window and the exports as buttons, over the gauge panel
 * (gauge.tsx). Gated by a development bundle and the `show_ledger`
 * setting — off by default.
 *
 * It also owns the dev lifecycle the ledger's surfaces share, which runs
 * whether or not the panel is showing: opening the kernel's recording window,
 * which is off until something asks for it; the long-frame observers, so the
 * ring is already warm when the panel is opened; and the handles on `window`
 * an A/B is driven and read through from the console — `__pxl_ledger_trace`,
 * `__pxl_view` and `__pxl_gauge`. Two builds are only comparable read at
 * the same view, which is what `__pxl_view` puts the camera at (INPUT_VIEW),
 * and the numbers are the panel's own.
 */

import { useEffect, useState } from "react";
import { Pressable, StyleSheet, Text, useWindowDimensions, View } from "react-native";

import { alpha, color } from "@/shared/ui/theme.ts";
import { gauge, LedgerGauge, Rule } from "./gauge.tsx";
import { ledgerDrainLog, ledgerLogLongFrames } from "./log.ts";
import { ledgerReportPrint, ledgerReportText } from "./report.ts";
import { type Gauge, ledgerGaugeRead, ledgerReportRead } from "./view.ts";
import { installLongFrameObservers, longFramesClear } from "./long-frame.ts";
import { ledgerTraceDownload } from "./trace.ts";
import { Command, encode_input_view, encode_ledger_log } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, useKernel } from "@/kernel/shell.ts";
import { SETTINGS } from "@/feature/setting/schema.ts";
import { DEV_BUNDLE } from "@/config/bundle.ts";

/** How often the stream is collected while it is on. */
const DRAIN_MS = 500;

/** A button's tint: plain; go, when a window is stopped and one can begin;
 * halt, while one is open; off, when there is nothing to press it for. */
type Tint = "plain" | "go" | "halt" | "off";

/** The instrument's own greens and ambers, Tailwind's as the panel's were. */
const EMERALD_300 = "#5ee9b5";
const EMERALD_400 = "#00d492";
const AMBER_300 = "#ffd230";

/** Each tint's border, ground and ink, and the ground and ink it lights to
 * under a hovering pointer or a finger. */
const TINT: Record<Tint, { border: string; ground: string; ink: string; lit: string; litInk: string }> = {
  plain: { border: alpha(color.white, 0.15), ground: alpha(color.white, 0.05), ink: alpha(color.white, 0.6), lit: alpha(color.white, 0.1), litInk: alpha(color.white, 0.9) },
  go: { border: alpha(EMERALD_400, 0.4), ground: alpha(EMERALD_400, 0.1), ink: EMERALD_300, lit: alpha(EMERALD_400, 0.2), litInk: EMERALD_300 },
  halt: { border: alpha(color.amber400, 0.4), ground: alpha(color.amber400, 0.1), ink: AMBER_300, lit: alpha(color.amber400, 0.2), litInk: AMBER_300 },
  off: { border: alpha(color.white, 0.1), ground: alpha(color.white, 0.05), ink: alpha(color.white, 0.25), lit: alpha(color.white, 0.05), litInk: alpha(color.white, 0.25) },
};

/** A button whose hint is said in the panel's title while a pointer is over
 * it or a finger holds it: a page's tooltip would reach no device, and a
 * hold is a device's hover. */
function PanelButton({ label, hint, tint = "plain", onPress, onHint, disabled }: {
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
      {({ pressed }) => <Text style={[gauge.text, { color: hovered || pressed ? t.litInk : t.ink }]}>{label}</Text>}
    </Pressable>
  );
}

/** How often the panel repaints. */
const POLL_MS = 500;

/** The panel reads the recent past, not the whole recording. Folding every
 * frame since load on each repaint grew into a 60 ms task every half second
 * two minutes in — the panel was the stall it was showing. The report still
 * covers the whole window. */
const LIVE_WINDOW_MS = 10_000;

type LedgerGlobals = typeof globalThis & {
  __pxl_ledger_trace?: () => Promise<void>;
  __pxl_view?: (dist: number | null, polar: number | null) => void;
  __pxl_gauge?: (window_ms: number) => Gauge;
};

/** A fresh window on both sides: the kernel's rings and the shell's long frames. */
function startWindow(): void {
  kernel_command(Command.LEDGER_START);
  longFramesClear();
}

export function LedgerOverlay() {
  const show = useKernel((s) => DEV_BUNDLE && s.settings[SETTINGS.show_ledger.key] === 1);
  const [logging, setLogging] = useState(false);
  const [hint, setHint] = useState<string | null>(null);
  const [g, setG] = useState<Gauge | null>(null);
  const { height } = useWindowDimensions();

  useEffect(() => {
    if (!show) return;
    const read = () => setG(ledgerGaugeRead(LIVE_WINDOW_MS));
    read();
    const id = setInterval(read, POLL_MS);
    return () => clearInterval(id);
  }, [show]);

  // Nothing is pushed: the kernel keeps the rows and this comes for them,
  // in batches rather than per frame — the stream is read while watching, and
  // a drain that ran on every frame would be instrumentation paced by the
  // thing it measures. The buffer is capped, so a slow reader loses the
  // oldest rather than growing the kernel's heap.
  useEffect(() => {
    if (!logging) return;
    const id = setInterval(ledgerDrainLog, DRAIN_MS);
    return () => clearInterval(id);
  }, [logging]);

  useEffect(() => {
    if (!DEV_BUNDLE) return;
    // The kernel records nothing until something asks, so that a client with no
    // overlay pays for none of it. This is the asking, and it happens on mount
    // rather than when the panel is opened: what makes a ledger worth having is
    // that the minute before you went looking is already in it.
    startWindow();
    // The observers outlive the panel — they are the ledger's, not its, so
    // closing the panel does not stop a window being recorded — but they do
    // not outlive dev: a client with no overlay has no one to read what they
    // keep.
    installLongFrameObservers();
    const w = globalThis as LedgerGlobals;
    w.__pxl_ledger_trace = ledgerTraceDownload;
    w.__pxl_view = (dist, polar) => kernel_command(Command.INPUT_VIEW, encode_input_view({ dist, polar }));
    w.__pxl_gauge = (window_ms) => ledgerGaugeRead(window_ms);
    return () => {
      delete w.__pxl_ledger_trace;
      delete w.__pxl_view;
      delete w.__pxl_gauge;
    };
  }, []);

  if (!DEV_BUNDLE || !show) return null;
  const recording = g?.rec.recording === true;

  return (
    <View style={[styles.panel, { maxHeight: height - 96 }]}>
      <Text style={[gauge.text, styles.title]}>{hint ?? "sched/ledger, last 10s"}</Text>

      {
        /* The buttons come before the numbers: on a phone the panel runs past
          the bottom of the screen, and what must stay reachable is start/stop
          and the exports. The body below is clipped rather than scrolled — the
          panel lets gestures through to the map, and a scrolling body would
          not. */
      }
      <View style={styles.buttons}>
        {
          /* start and stop are both present rather than one toggle: the dev
            overlay opens a window on mount so a cold-load stall is never
            missed, so a toggle would read "stop" until you had stopped once —
            and then "start" is the button you cannot find when you want a
            clean window. */
        }
        <PanelButton onHint={setHint} label="start" hint="clear everything and begin a fresh window" tint={recording ? "plain" : "go"} onPress={startWindow} />
        <PanelButton
          onHint={setHint}
          label="stop"
          hint="freeze the window so exports are stable while you read them"
          tint={recording ? "halt" : "off"}
          disabled={!recording}
          onPress={() => kernel_command(Command.LEDGER_STOP)}
        />
      </View>
      <View style={styles.buttons}>
        <PanelButton onHint={setHint} label="report" hint="print + copy a text report" onPress={() => void ledgerReportPrint(ledgerReportText(ledgerReportRead()))} />
        <PanelButton onHint={setHint} label="trace" hint="save the window as a Trace Event Format file for ui.perfetto.dev" onPress={() => void ledgerTraceDownload()} />
        <PanelButton
          onHint={setHint}
          label={logging ? "log on" : "log"}
          hint="stream each settled task, missed frame and long frame to the console"
          onPress={() => {
            kernel_command(Command.LEDGER_LOG, encode_ledger_log({ on: !logging }));
            ledgerLogLongFrames(!logging);
            setLogging(!logging);
          }}
        />
      </View>
      <Rule />

      <View style={{ pointerEvents: "none" }}>
        <LedgerGauge g={g} />
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  // Under the settings button, and over the map's chrome.
  panel: {
    position: "absolute",
    left: 8,
    top: 84,
    zIndex: 50,
    width: 228,
    overflow: "hidden",
    borderRadius: 8,
    padding: 10,
    backgroundColor: alpha(color.black, 0.72),
    pointerEvents: "box-none",
  },
  title: { marginBottom: 4, fontWeight: "600", letterSpacing: 0.25, color: alpha(color.white, 0.55) },
  buttons: { flexDirection: "row", gap: 4, marginTop: 4 },
  button: { flex: 1, alignItems: "center", paddingVertical: 4, borderWidth: 1, borderRadius: 4 },
});
