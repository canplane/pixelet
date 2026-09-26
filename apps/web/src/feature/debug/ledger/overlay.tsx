// @pxl/web/feature/debug/ledger/overlay.tsx

/**
 * The debug overlay for the kernel's ledger
 * (crates/pxl/web/kernel/src/sched/ledger): the recording window and the
 * exports as buttons, over the gauge panel (gauge.tsx). Shown by the
 * `show_ledger` setting, off by default, in any build: it is the debug
 * menu's, and a release is what it most needs to measure.
 *
 * The ledger records while the panel is on and not otherwise, so a client
 * that never turns it on pays for none of it. The setting is kept, so one
 * left on records from the boot — what makes a ledger worth having is that
 * the minute before you went looking is already in it. With the panel come
 * the long-frame observers and the handles on `window` an A/B is driven and
 * read through from the console — `__pxl_ledger_trace`, `__pxl_view` and
 * `__pxl_gauge`. Two builds are only comparable read at the same view, which
 * is what `__pxl_view` puts the camera at (INPUT_VIEW), and the numbers are
 * the panel's own.
 */

import { ChevronDown, ChevronUp } from "lucide-react-native";
import { useEffect, useState } from "react";
import { Pressable, ScrollView, StyleSheet, Text, View } from "react-native";

import { alpha, color } from "@/shared/ui/theme.ts";
import { gauge, Head, LedgerGauge } from "./gauge.tsx";
import { ledgerDrainLog, ledgerLogLongFrames } from "./log.ts";
import { ledgerReportPrint, ledgerReportText } from "./report.ts";
import { type Gauge, ledgerGaugeRead, ledgerReportRead } from "./view.ts";
import { installLongFrameObservers, longFramesClear } from "./long-frame.ts";
import { ledgerTraceDownload } from "./trace.ts";
import { Command, encode_input_view, encode_ledger_log } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, useKernel } from "@/kernel/shell.ts";
import { SETTINGS } from "@/feature/setting/schema.ts";

/** How often the stream is collected while it is on. */
const DRAIN_MS = 500;

/** A button's tint: plain; go, when a window is stopped and one can begin;
 * halt, while one is open; off, when there is nothing to press it for. */
export const enum Tint {
  Plain = 0,
  Go = 1,
  Halt = 2,
  Off = 3,
}

/** Each tint's border, ground and ink, and the ground and ink it lights to
 * under a hovering pointer or a finger. */
const TINT: Record<Tint, { border: string; ground: string; ink: string; lit: string; litInk: string }> = {
  [Tint.Plain]: { border: alpha(color.white, 0.15), ground: alpha(color.white, 0.05), ink: alpha(color.white, 0.6), lit: alpha(color.white, 0.1), litInk: alpha(color.white, 0.9) },
  [Tint.Go]: { border: alpha(color.ledgerGo, 0.4), ground: alpha(color.ledgerGo, 0.1), ink: color.ledgerGoInk, lit: alpha(color.ledgerGo, 0.2), litInk: color.ledgerGoInk },
  [Tint.Halt]: { border: alpha(color.glow, 0.4), ground: alpha(color.glow, 0.1), ink: color.ledgerHaltInk, lit: alpha(color.glow, 0.2), litInk: color.ledgerHaltInk },
  [Tint.Off]: { border: alpha(color.white, 0.1), ground: alpha(color.white, 0.05), ink: alpha(color.white, 0.25), lit: alpha(color.white, 0.05), litInk: alpha(color.white, 0.25) },
};

/** A button whose hint is said under the buttons while a pointer is over
 * it or a finger holds it: a page's tooltip would reach no device, and a
 * hold is a device's hover. */
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
  const show = useKernel((s) => s.settings[SETTINGS.show_ledger.key] === 1);
  const [logging, setLogging] = useState(false);
  const [hint, setHint] = useState<string | null>(null);
  const [g, setG] = useState<Gauge | null>(null);
  const [open, setOpen] = useState(true);

  // Folded, the panel keeps recording and stops reading: the gauge is what
  // the fold of the window is for, and nobody is looking at it.
  useEffect(() => {
    if (!show || !open) return;
    const read = () => setG(ledgerGaugeRead(LIVE_WINDOW_MS));
    read();
    const id = setInterval(read, POLL_MS);
    return () => clearInterval(id);
  }, [show, open]);

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
    if (!show) return;
    startWindow();
    // The observers, once installed, stay: the browser offers no way to
    // take one back but disconnecting it, and a panel turned on again
    // wants the ring they fill.
    installLongFrameObservers();
    const w = globalThis as LedgerGlobals;
    w.__pxl_ledger_trace = ledgerTraceDownload;
    w.__pxl_view = (dist, polar) => kernel_command(Command.INPUT_VIEW, encode_input_view({ dist, polar }));
    w.__pxl_gauge = (window_ms) => ledgerGaugeRead(window_ms);
    return () => {
      kernel_command(Command.LEDGER_STOP);
      kernel_command(Command.LEDGER_LOG, encode_ledger_log({ on: false }));
      ledgerLogLongFrames(false);
      setLogging(false);
      delete w.__pxl_ledger_trace;
      delete w.__pxl_view;
      delete w.__pxl_gauge;
    };
  }, [show]);

  if (!show) return null;
  const recording = g?.rec.recording === true;

  return (
    <View style={styles.panel}>
      {
        /* The buttons come before the numbers, so start/stop and the exports
          stay put while the numbers scroll under them, and are what is left
          when the panel is folded to leave a phone's map in view. */
      }
      <View style={styles.buttons}>
        {
          /* start and stop are both present rather than one toggle: the panel
            opens a window when it is shown so a cold-load stall is never
            missed, so a toggle would read "stop" until you had stopped once —
            and then "start" is the button you cannot find when you want a
            clean window. */
        }
        <PanelButton onHint={setHint} label="start" hint="clear everything and begin a fresh window" tint={recording ? Tint.Plain : Tint.Go} onPress={startWindow} />
        <PanelButton
          onHint={setHint}
          label="stop"
          hint="freeze the window so exports are stable while you read them"
          tint={recording ? Tint.Halt : Tint.Off}
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
      {
        /* Under the buttons, not over them: a hint that ran to a second line
          above them moved the button the pointer was on out from under it,
          which took the hint away and brought the button back. */
      }
      {hint !== null && <Text style={[gauge.text, styles.hint]}>{hint}</Text>}

      <Pressable onPress={() => setOpen(!open)} accessibilityRole="button" aria-expanded={open}>
        <Head end={open ? <ChevronUp size={12} color={HINT_INK} /> : <ChevronDown size={12} color={HINT_INK} />}>last 10s</Head>
      </Pressable>
      {open && (
        <ScrollView style={styles.body} showsVerticalScrollIndicator={false}>
          <LedgerGauge g={g} />
        </ScrollView>
      )}
    </View>
  );
}

const HINT_INK = alpha(color.white, 0.55);

const styles = StyleSheet.create({
  // As tall as its rows, and no taller than the room the map leaves it
  // (view.tsx), which the body scrolls within.
  panel: {
    flexShrink: 1,
    width: 228,
    overflow: "hidden",
    borderRadius: 8,
    padding: 10,
    backgroundColor: alpha(color.black, 0.72),
    pointerEvents: "box-none",
  },
  hint: { marginTop: 6, fontWeight: "600", letterSpacing: 0.25, color: HINT_INK },
  // Shrinks to what the panel's height leaves it, and scrolls the rest.
  body: { flexShrink: 1 },
  buttons: { flexDirection: "row", gap: 4, marginTop: 4 },
  button: { flex: 1, alignItems: "center", paddingVertical: 4, borderWidth: 1, borderRadius: 4 },
});
