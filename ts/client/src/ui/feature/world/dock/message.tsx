// @pxl/client/ui/feature/world/dock/message.tsx

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, type TextInput, View } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Glyph } from "@/ui/shared/component/glyph.tsx";
import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import { Presence } from "@/ui/shared/component/motion/presence.tsx";
import { FADE, SNAP } from "@/ui/shared/component/motion/transition.ts";
import { put_keyboard_away } from "@/ui/shared/component/keyboard.tsx";
import { LABEL } from "@/ui/shared/component/palette.ts";
import { TextField } from "@/ui/shared/component/text-field.tsx";
import {
  CHROME,
  COLOR,
  DISABLED_CONTENT,
} from "@/ui/shared/component/theme.ts";
import { useCore } from "@/ui/shared/watch.ts";
import { append_chat_line, DockMode, set_dock_mode } from "./store.ts";
import { SLOT } from "./strip.tsx";
import * as edit from "@/service/core/edit.ts";
import * as snapshot from "@/service/core/snapshot.ts";

/** The dock holds this state, so that it is there whether or not the cards are
 * drawn yet. */
export function useMessage(mode: DockMode) {
  const drafting = useCore((s) => s.draft !== null);
  const [message_text, set_message_text] = useState("");
  const [marker_fits, set_marker_fits] = useState(true);
  const [is_sending, set_is_sending] = useState(false);
  const input_ref = useRef<TextInput | null>(null);

  // The keyboard is up exactly while the dock is in message mode. A marker's
  // draft exists only in message mode, so leaving the mode cancels it.
  useEffect(() => {
    if (mode === DockMode.MESSAGE) input_ref.current?.focus();
    else {
      put_keyboard_away();
      if (drafting) edit.marker_cancel();
    }
  }, [mode]);

  // When a draft opens, the dock turns to message mode and the field takes the
  // draft's text. From then on the field owns the text and sends the core each
  // change. When the draft closes, the field is emptied for the chat.
  useEffect(() => {
    set_message_text(drafting ? (snapshot.cur().draft ?? "") : "");
    set_marker_fits(true);
    if (drafting) set_dock_mode(DockMode.MESSAGE);
  }, [drafting]);

  function change_text(text: string) {
    set_message_text(text);
    if (drafting) {
      void edit.marker_text(text.trim()).then(
        (fits) => set_marker_fits(fits),
        () => set_marker_fits(false),
      );
    }
  }

  function send_message() {
    const text = message_text.trim();
    if (!can_send || !fits || is_sending) return;
    set_is_sending(true);
    setTimeout(() => {
      set_message_text("");
      set_is_sending(false);
      if (!drafting) {
        append_chat_line(text);
        input_ref.current?.focus();
        return;
      }
      edit.marker_commit();
      set_dock_mode(DockMode.PALETTE);
    }, FADE.duration_ms);
  }

  const can_send = message_text.trim() !== "";
  // A marker's text the core refused cannot be committed. A chat line too
  // long for the link is refused whole, never cut short.
  const fits = drafting ? marker_fits : true;

  return {
    drafting,
    message_text,
    is_sending,
    input_ref,
    change_text,
    send_message,
    can_send,
    fits,
  };
}

export function Message({
  message,
  width,
}: {
  message: ReturnType<typeof useMessage>;
  width: f64;
}) {
  const { t } = useTranslation();
  const {
    drafting,
    message_text,
    is_sending,
    input_ref,
    change_text,
    send_message,
    can_send,
    fits,
  } = message;

  return (
    <View style={[STYLE.message, { width }]}>
      <View style={STYLE.message_icon}>
        <Glyph glyph="bubble.left" size={CHROME.glyph} color={LABEL.primary} />
      </View>
      <Motion
        to={{ opacity: is_sending ? 0 : 1 }}
        transition={FADE}
        style={STYLE.field_fill}
      >
        <TextField
          ref={input_ref}
          value={message_text}
          onChangeText={change_text}
          onSubmit={send_message}
          placeholder={
            drafting
              ? t("world.dock.pin_placeholder")
              : t("world.dock.message_placeholder")
          }
          placeholderTextColor={LABEL.placeholder}
          style={STYLE.message_input}
        />
      </Motion>
      <Presence
        show={can_send}
        from={{ opacity: 0, scale: 0.5 }}
        exit={{ opacity: 0 }}
        transition={SNAP}
        style={STYLE.slot}
      >
        <Pressable
          onPress={send_message}
          disabled={is_sending || !fits}
          accessibilityRole="button"
          accessibilityLabel={
            drafting ? t("world.dock.put_pin") : t("world.dock.send")
          }
          style={({ pressed }) => [
            STYLE.send,
            drafting && STYLE.confirm,
            pressed && STYLE.send_pressed,
            !fits && STYLE.send_refused,
          ]}
        >
          <Glyph
            glyph={drafting ? "checkmark" : "arrow.up"}
            size={CHROME.glyph}
            weight="bold"
            color={COLOR.white}
          />
        </Pressable>
      </Presence>
    </View>
  );
}

const STYLE = StyleSheet.create({
  message: {
    flexDirection: "row",
    alignItems: "center",
    height: CHROME.control,
    paddingRight: (CHROME.control - CHROME.target) / 2,
  },
  message_icon: {
    width: CHROME.control,
    height: CHROME.control,
    alignItems: "center",
    justifyContent: "center",
  },
  field_fill: { flex: 1, minWidth: 0 },
  message_input: { width: "100%", color: LABEL.primary },
  send: {
    width: CHROME.selection,
    height: CHROME.selection,
    borderRadius: CHROME.selection / 2,
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: COLOR.send,
  },
  confirm: { backgroundColor: COLOR.confirm },
  send_pressed: { opacity: 0.7 },
  send_refused: { opacity: DISABLED_CONTENT },
  slot: SLOT,
});
