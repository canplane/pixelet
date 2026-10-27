// @pxl/client/shell/debug/profiler.tsx

/**
 * The profiler panel: the profiler's recording controls, exports and
 * gauge, and its own settings (section `profiler`) under the buttons. Shown by the `profiler`
 * setting in any build; the kernel records from boot while it is on. A development page reads the same from
 * the console (shell/debug/console.ts).
 */

import { CheckSquareIcon, SquareIcon } from "phosphor-react-native";
import { useEffect, useState } from "react";
import { Pressable, ScrollView, StyleSheet, Text, View } from "react-native";

import { alpha, color, disabledContent } from "@/shell/ui/theme.ts";
import { gauge, ProfilerGauge, ProfilerRec } from "./profiler/gauge.tsx";
import { Fold, HINT_INK } from "./fold.tsx";
import { n1 } from "./profiler/lane.ts";
import { profilerReportCopy, profilerReportText } from "./profiler/report.ts";
import { fps, profilerReportRead } from "./profiler/view.ts";
import { LIVE_WINDOW_MS, useProfilerGauge } from "./profiler/poll.ts";
import {
  installLongFrameObservers,
  longFramesClear,
} from "./profiler/long-frame.ts";
import { profilerTraceSave } from "./profiler/trace.ts";
import { Command } from "@/abi/cmd.spec.ts";
import { kernel_command, kernel_setting } from "@/abi/shell.ts";
import {
  type SettingName,
  SETTINGS,
  settingsIn,
} from "@/shell/setting/schema.ts";
import { useKernel } from "@/shell/store/watch.ts";

const enum Tint {
  Plain = 0,
  Go = 1,
  Halt = 2,
  Off = 3,
}

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
  [Tint.Plain]: {
    border: alpha(color.white, 0.15),
    ground: alpha(color.white, 0.05),
    ink: alpha(color.white, 0.6),
    lit: alpha(color.white, 0.1),
    litInk: alpha(color.white, 0.9),
  },
  [Tint.Go]: hued(color.profilerGo),
  [Tint.Halt]: hued(color.profilerHalt),
  [Tint.Off]: {
    border: alpha(color.white, 0.1),
    ground: alpha(color.white, 0.05),
    ink: alpha(color.white, 0.25),
    lit: alpha(color.white, 0.05),
    litInk: alpha(color.white, 0.25),
  },
};

/** The hint shows under the buttons on hover or hold, since a device has no tooltip. */
function PanelButton(
  { label, hint, tint = Tint.Plain, onPress, onHint, disabled }: {
    label: string;
    hint: string;
    tint?: Tint;
    onPress: () => void;
    onHint: (hint: string | null) => void;
    disabled?: boolean;
  },
) {
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
      style={(
        { pressed },
      ) => [styles.button, {
        borderColor: t.border,
        backgroundColor: hovered || pressed ? t.lit : t.ground,
      }]}
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

/** A setting the panel holds (section `profiler`). */
type Own = {
  [K in SettingName]: (typeof SETTINGS)[K]["section"] extends "profiler" ? K
    : never;
}[SettingName];

/** Each a checkbox under the buttons, set while the window is stopped: one
 * that changed mid-window would leave the window's frames counted apart. A
 * dev one is absent from a production build (`settingsIn`), and its box
 * with it. */
const OWN: Record<Own, { label: string; note: string }> = {
  ray_counts: { label: "Ray counts", note: "into the report; slows the casts" },
};

function OwnCheck({ name, disabled }: { name: Own; disabled: boolean }) {
  const { key } = SETTINGS[name];
  const on = useKernel((s) => s.settings[key] === 1);
  const own = OWN[name];
  const Box = on ? CheckSquareIcon : SquareIcon;
  return (
    <Pressable
      onPress={() => kernel_setting(key, on ? 0 : 1)}
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
  kernel_command(Command.PROFILER_START);
  longFramesClear();
}

export function ProfilerPanel() {
  const show = useKernel((s) => s.settings[SETTINGS.profiler.key] === 1);
  const [hint, setHint] = useState<string | null>(null);
  const [said, setSaid] = useState<string | null>(null);
  // Read while shown, folded or not: the title carries the fps.
  const g = useProfilerGauge(show);

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
  const f = g?.frames ?? null;
  const rate = f === null ? null : fps(f);
  const title = rate === null ? "Profiler" : `Profiler · ${n1(rate)} fps`;

  return (
    <Fold
      name="profiler"
      title={title}
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
          tint={recording ? Tint.Plain : Tint.Go}
          onPress={restartWindow}
        />
        <PanelButton
          onHint={setHint}
          label="Stop"
          hint="freeze the window so exports are stable while you read them"
          tint={recording ? Tint.Halt : Tint.Off}
          disabled={!recording}
          onPress={() => kernel_command(Command.PROFILER_STOP)}
        />
        <PanelButton
          onHint={setHint}
          label="Report"
          hint="copy the window's summary as text"
          onPress={() =>
            void profilerReportCopy(profilerReportText(profilerReportRead()))
              .then(setSaid)}
        />
        <PanelButton
          onHint={setHint}
          label="Trace"
          hint="save the window as a timeline for ui.perfetto.dev"
          onPress={() => void profilerTraceSave().then(setSaid)}
        />
      </View>
      {(settingsIn("profiler") as Own[]).map((name) => (
        <OwnCheck key={name} name={name} disabled={recording} />
      ))}
      {/* Under the buttons: a two-line hint above them moved the hovered button away. */}
      {(said ?? hint) !== null && (
        <Text style={[gauge.text, styles.hint]}>{said ?? hint}</Text>
      )}
      {g !== null && <ProfilerRec rec={g.rec} />}
      <ScrollView style={styles.body} showsVerticalScrollIndicator={false}>
        <ProfilerGauge g={g} />
      </ScrollView>
    </Fold>
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
