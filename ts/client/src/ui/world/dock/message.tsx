// @pxl/client/ui/world/dock/message.tsx

/** The message card. Its field types a chat line and sends it, or, while a
 * marker is being drafted, writes the marker's text. */

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, type TextInput, View } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Glyph } from "@/ui/component/glyph.tsx";
import { Motion } from "@/ui/component/motion/motion.tsx";
import { Presence } from "@/ui/component/motion/presence.tsx";
import { FADE, SNAP } from "@/ui/component/motion/transition.ts";
import { putKeyboardAway } from "@/ui/component/keyboard.tsx";
import { LABEL } from "@/ui/component/palette.ts";
import { TextField } from "@/ui/component/text-field.tsx";
import { chrome, color, disabledContent } from "@/ui/component/theme.ts";
import { useCore } from "@/ui/watch.ts";
import { appendChatLine, DockMode, setDockMode } from "./store.ts";
import { SLOT } from "./strip.tsx";
import * as edit from "@/service/core/edit.ts";
import * as scene from "@/service/core/scene.ts";
import * as snapshot from "@/service/core/snapshot.ts";

/** The message field's text and its sending. The dock holds this state, so
 * that it is there whether or not the cards are drawn yet. */
export function useMessage(mode: DockMode) {
  // While the core holds a marker's draft, the field writes the marker's text
  // instead of a chat line.
  const drafting = useCore((s) => s.draft !== null);
  const [messageText, setMessageText] = useState("");
  const [markerFits, setMarkerFits] = useState(true);
  const [isSending, setIsSending] = useState(false);
  const inputRef = useRef<TextInput | null>(null);

  // The keyboard is up exactly while the dock is in message mode. A marker's
  // draft exists only in message mode, so leaving the mode cancels it.
  useEffect(() => {
    if (mode === DockMode.Message) inputRef.current?.focus();
    else {
      putKeyboardAway();
      if (drafting) edit.marker_cancel();
    }
  }, [mode]);

  // When a draft opens, the dock turns to message mode and the field takes the
  // draft's text. From then on the field owns the text and sends the core each
  // change. When the draft closes, the field is emptied for the chat.
  useEffect(() => {
    setMessageText(drafting ? (snapshot.current().draft ?? "") : "");
    setMarkerFits(true);
    if (drafting) setDockMode(DockMode.Message);
  }, [drafting]);

  function changeText(text: string) {
    setMessageText(text);
    if (drafting) {
      void edit.marker_text(text.trim()).then((r) => setMarkerFits(r >= 0));
    }
  }

  function sendMessage() {
    const text = messageText.trim();
    if (!canSend || !fits || isSending) return;
    setIsSending(true);
    setTimeout(() => {
      setMessageText("");
      setIsSending(false);
      if (!drafting) {
        appendChatLine(text);
        inputRef.current?.focus();
        return;
      }
      edit.marker_commit();
      // With the marker placed, the dock returns to the palette.
      setDockMode(DockMode.Palette);
    }, FADE.durationMs);
  }

  const canSend = messageText.trim() !== "";
  // The core refuses a line too long for it and never cuts one short, so a
  // line that does not fit cannot be sent.
  const fits = drafting ? markerFits : scene.chat_fits(messageText.trim());

  return {
    drafting,
    messageText,
    isSending,
    inputRef,
    changeText,
    sendMessage,
    canSend,
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
    messageText,
    isSending,
    inputRef,
    changeText,
    sendMessage,
    canSend,
    fits,
  } = message;

  return (
    <View style={[styles.message, { width }]}>
      <View style={styles.messageIcon}>
        <Glyph glyph="bubble.left" size={chrome.glyph} color={LABEL.primary} />
      </View>
      <Motion
        to={{ opacity: isSending ? 0 : 1 }}
        transition={FADE}
        style={styles.fieldFill}
      >
        <TextField
          ref={inputRef}
          value={messageText}
          onChangeText={changeText}
          onSubmit={sendMessage}
          placeholder={
            drafting
              ? t("world.dock.pin_placeholder")
              : t("world.dock.message_placeholder")
          }
          placeholderTextColor={LABEL.placeholder}
          style={styles.messageInput}
        />
      </Motion>
      <Presence
        show={canSend}
        from={{ opacity: 0, scale: 0.5 }}
        exit={{ opacity: 0 }}
        transition={SNAP}
        style={styles.slot}
      >
        <Pressable
          onPress={sendMessage}
          disabled={isSending || !fits}
          accessibilityRole="button"
          accessibilityLabel={
            drafting ? t("world.dock.pin_done") : t("world.dock.send")
          }
          style={({ pressed }) => [
            styles.send,
            drafting && styles.confirm,
            pressed && styles.sendPressed,
            !fits && styles.sendRefused,
          ]}
        >
          <Glyph
            glyph={drafting ? "checkmark" : "arrow.up"}
            size={chrome.glyph}
            weight="bold"
            color={color.white}
          />
        </Pressable>
      </Presence>
    </View>
  );
}

const styles = StyleSheet.create({
  message: {
    flexDirection: "row",
    alignItems: "center",
    height: chrome.control,
    paddingRight: (chrome.control - chrome.target) / 2,
  },
  messageIcon: {
    width: chrome.control,
    height: chrome.control,
    alignItems: "center",
    justifyContent: "center",
  },
  fieldFill: { flex: 1, minWidth: 0 },
  messageInput: { width: "100%", color: LABEL.primary },
  send: {
    width: chrome.selection,
    height: chrome.selection,
    borderRadius: chrome.selection / 2,
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: color.send,
  },
  confirm: { backgroundColor: color.confirm },
  sendPressed: { opacity: 0.7 },
  sendRefused: { opacity: disabledContent },
  slot: SLOT,
});
