// @pxl/client/shell/debug/perf.tsx

/**
 * The profiler panel: the profiler's recording controls, exports and
 * gauge, and its own settings (section `perf`) under the buttons. Shown by the `perf`
 * setting in any build; the kernel records from boot while it is on. A development page reads the same from
 * the console (shell/debug/console.ts).
 */

import { CheckSquareIcon, SquareIcon } from "phosphor-react-native";
import { useEffect, useState } from "react";
import { Pressable, ScrollView, StyleSheet, Text, View } from "react-native";

import { alpha, color, disabledContent } from "@/shell/ui/theme.ts";
import { gauge, PerfGauge, PerfRec } from "./perf/gauge.tsx";
import { Accordion, HINT_INK } from "./accordion.tsx";
import { n1 } from "./perf/column.ts";
import { perfReportCopy, perfReportText } from "./perf/report.ts";
import { movingFps, perfReportRead } from "./perf/view.ts";
import { LIVE_WINDOW_MS, usePerfGauge } from "./perf/poll.ts";
import {
  installLongFrameObservers,
  longFramesClear,
} from "./perf/long-frame.ts";
import { perfTraceSave } from "./perf/trace.ts";
import {
  SETTING,
  type SettingName,
  settingsIn,
} from "@/shell/setting/schema.ts";
import { useKernel } from "@/shell/store/watch.ts";
import * as perf from "@/port/perf.ts";
import * as setting from "@/port/setting.ts";

const PLAIN = 0;
const GO = 1;
const HALT = 2;
const OFF = 3;
type Tint = typeof PLAIN | typeof GO | typeof HALT | typeof OFF;

interface TintLook {
  border: string;
  ground: string;
  ink: string;
  lit: string;
  litInk: string;
}

function hued(hue: string): TintLook {
  return {
    border: alpha(hue, 0.4),
    ground: alpha(hue, 0.1),
    ink: hue,
    lit: alpha(hue, 0.2),
    litInk: hue,
  };
}

const TINT: Record<Tint, TintLook> = {
  [PLAIN]: {
    border: alpha(color.white, 0.15),
    ground: alpha(color.white, 0.05),
    ink: alpha(color.white, 0.6),
    lit: alpha(color.white, 0.1),
    litInk: alpha(color.white, 0.9),
  },
  [GO]: hued(color.perfGo),
  [HALT]: hued(color.perfHalt),
  [OFF]: {
    border: alpha(color.white, 0.1),
    ground: alpha(color.white, 0.05),
    ink: alpha(color.white, 0.25),
    lit: alpha(color.white, 0.05),
    litInk: alpha(color.white, 0.25),
  },
};

/** The hint shows under the buttons on hover or hold, since a device has no tooltip. */
function PanelButton({
  label,
  hint,
  tint = PLAIN,
  onPress,
  onHint,
  disabled,
}: {
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
      style={({ pressed }) => [
        styles.button,
        {
          borderColor: t.border,
          backgroundColor: hovered || pressed ? t.lit : t.ground,
        },
      ]}
    >
      {({ pressed }) => (
        <Text
          style={[
            gauge.text,
            { color: hovered || pressed ? t.litInk : t.ink },
            disabled && { opacity: disabledContent },
          ]}
        >
          {label}
        </Text>
      )}
    </Pressable>
  );
}

const SAID_MS = 2500;

/** A setting the panel holds (section `perf`). */
type Own = {
  [K in SettingName]: (typeof SETTING)[K]["section"] extends "perf" ? K : never;
}[SettingName];

/** Each a checkbox under the buttons, set while the window is stopped: one
 * that changed mid-window would leave the window's frames counted apart. A
 * dev one is absent from a production build (`settingsIn`), and its box
 * with it. */
const OWN: Record<Own, { label: string; note: string }> = {
  ray_counts: { label: "Ray counts", note: "into the report; slows the casts" },
};

function OwnCheck({ name, disabled }: { name: Own; disabled: boolean }) {
  const { key } = SETTING[name];
  const on = useKernel((s) => s.settings[key] === 1);
  const own = OWN[name];
  const Box = on ? CheckSquareIcon : SquareIcon;
  return (
    <Pressable
      onPress={() => setting.set(key, on ? 0 : 1)}
      disabled={disabled}
      accessibilityRole="checkbox"
      aria-checked={on}
      style={[
        gauge.row,
        styles.check,
        disabled && { opacity: disabledContent },
      ]}
    >
      <View style={styles.checkLabel}>
        <Box
          size={18}
          color={gauge.text.color}
          weight={on ? "fill" : "regular"}
        />
        <Text style={gauge.text}>{own.label}</Text>
      </View>
      <Text style={[gauge.text, gauge.key, styles.note]} numberOfLines={1}>
        {disabled ? "stop the window to change" : own.note}
      </Text>
    </Pressable>
  );
}

function restartWindow(): void {
  perf.start();
  longFramesClear();
}

export function PerfPanel() {
  const show = useKernel((s) => s.settings[SETTING.perf.key] === 1);
  const [hint, setHint] = useState<string | null>(null);
  const [said, setSaid] = useState<string | null>(null);
  // Read while shown, folded or not: the title carries the fps.
  const g = usePerfGauge(show);

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
  }, [show]);

  if (!show) return null;
  const recording = g?.rec.recording === true;
  // The rate frames are drawn at over the last second, idle where recording
  // saw none move in it.
  const r = g?.recent ?? null;
  const rate = r === null ? null : movingFps(r);
  const note =
    rate !== null ? `${n1(rate)} fps` : recording ? "idle" : undefined;

  return (
    <Accordion
      name="perf"
      title="Performance"
      note={note}
      end={
        <Text style={[gauge.text, gauge.key]}>
          Last {LIVE_WINDOW_MS / 1000} s
        </Text>
      }
    >
      <View style={styles.buttons}>
        {/* Both, not a toggle: a window opened at boot would read "Stop" until first stopped. */}
        <PanelButton
          onHint={setHint}
          label={recording ? "Restart" : "Start"}
          hint="clear everything and begin a fresh window"
          tint={recording ? PLAIN : GO}
          onPress={restartWindow}
        />
        <PanelButton
          onHint={setHint}
          label="Stop"
          hint="freeze the window so exports are stable while you read them"
          tint={recording ? HALT : OFF}
          disabled={!recording}
          onPress={perf.stop}
        />
        <PanelButton
          onHint={setHint}
          label="Report"
          hint="copy the window's summary as text"
          onPress={() =>
            void perfReportCopy(perfReportText(perfReportRead())).then(setSaid)
          }
        />
        <PanelButton
          onHint={setHint}
          label="Trace"
          hint="save the window as a timeline for ui.perfetto.dev"
          onPress={() => void perfTraceSave().then(setSaid)}
        />
      </View>
      {(settingsIn("perf") as Own[]).map((name) => (
        <OwnCheck key={name} name={name} disabled={recording} />
      ))}
      {/* Under the buttons: a two-line hint above them moved the hovered button away. */}
      {(said ?? hint) !== null && (
        <Text style={[gauge.text, styles.hint]}>{said ?? hint}</Text>
      )}
      {g !== null && <PerfRec rec={g.rec} />}
      <ScrollView style={styles.body} showsVerticalScrollIndicator={false}>
        <PerfGauge g={g} />
      </ScrollView>
    </Accordion>
  );
}

const styles = StyleSheet.create({
  hint: {
    marginTop: 6,
    fontWeight: "600",
    letterSpacing: 0.25,
    color: HINT_INK,
  },
  body: { flexShrink: 1 },
  check: { alignItems: "center", marginTop: 6 },
  checkLabel: { flexDirection: "row", alignItems: "center", gap: 6 },
  note: { flexShrink: 1, textAlign: "right" },
  buttons: { flexDirection: "row", gap: 4, marginTop: 8 },
  button: {
    flex: 1,
    alignItems: "center",
    paddingVertical: 4,
    borderWidth: 1,
    borderRadius: 4,
  },
});
