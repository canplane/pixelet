// @pxl/client/shell/world/dock/message.tsx

/** The message's card: a line for the chat, or a marker draft's text, typed and sent. */

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, type TextInput, View } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Glyph } from "@/shell/ui/glyph.tsx";
import { Motion } from "@/shell/ui/motion/motion.tsx";
import { Presence } from "@/shell/ui/motion/presence.tsx";
import { FADE, SNAP } from "@/shell/ui/motion/transition.ts";
import { putKeyboardAway } from "@/shell/ui/keyboard.tsx";
import { LABEL } from "@/shell/ui/palette.ts";
import { TextField } from "@/shell/ui/text-field.tsx";
import { chrome, color, disabledContent } from "@/shell/ui/theme.ts";
import { useKernel } from "@/shell/store/watch.ts";
import { appendChatLine, DockMode, setDockMode } from "./store.ts";
import { SLOT } from "./strip.tsx";
import * as edit from "@/port/edit.ts";
import * as scene from "@/port/scene.ts";
import * as snapshot from "@/port/snapshot.ts";

/** The field's words and its sending, kept by the dock whether or not its cards are drawn yet. */
export function useMessage(mode: DockMode) {
  // A marker's draft turns the message card to writing it.
  const drafting = useKernel((s) => s.draft !== null);
  const [messageText, setMessageText] = useState("");
  const [markerFits, setMarkerFits] = useState(true);
  const [isSending, setIsSending] = useState(false);
  const inputRef = useRef<TextInput | null>(null);

  // The message mode is typing: the keyboard comes and goes with it, and a
  // marker's draft lives only inside it.
  useEffect(() => {
    if (mode === DockMode.Message) inputRef.current?.focus();
    else {
      putKeyboardAway();
      if (drafting) edit.marker_cancel();
    }
  }, [mode]);

  // A draft opens: the message comes up with its text, which from
  // then on is the field's, sent as typed; done, the field is the chat's again.
  useEffect(() => {
    setMessageText(drafting ? (snapshot.current().draft ?? "") : "");
    setMarkerFits(true);
    if (drafting) setDockMode(DockMode.Message);
  }, [drafting]);

  function changeText(text: string) {
    setMessageText(text);
    if (drafting) setMarkerFits(edit.marker_text(text.trim()) >= 0);
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
      // Put up, the palette comes back for the next.
      setDockMode(DockMode.Palette);
    }, FADE.durationMs);
  }

  const canSend = messageText.trim() !== "";
  // A line too long for the kernel is refused whole, never cut.
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
