// @pxl/client/shell/world/dock/message.tsx

/** The message's card: a line for the chat, or a waiting marker's words, typed and sent. */

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
import { cancelMarker, chatFits, DockMode, draftMarkerWords, markerTextFits, putMarker, useDockStore } from "@/shell/store/dock.ts";
import { kernel_snapshot } from "@/abi/shell.ts";
import { useKernel } from "@/shell/store/watch.ts";
import { SLOT } from "./strip.tsx";

/** The field's words and its sending, kept by the dock whether or not its cards are drawn yet. */
export function useMessage(mode: DockMode) {
  // A marker waiting for its words turns the message card to wording it.
  const wording = useKernel((s) => s.holding.marker);
  const appendChatLine = useDockStore.getState().appendChatLine;
  const [messageText, setMessageText] = useState("");
  const [isSending, setIsSending] = useState(false);
  const inputRef = useRef<TextInput | null>(null);

  // The message mode is typing: the keyboard comes and goes with it, and a
  // marker waiting for its words lives only inside it.
  useEffect(() => {
    if (mode === DockMode.Message) inputRef.current?.focus();
    else {
      putKeyboardAway();
      if (wording) cancelMarker();
    }
  }, [mode]);

  // A marker starts waiting: the message comes up with its words, which from
  // then on are the field's, sent as typed; done, the field is the chat's again.
  useEffect(() => {
    setMessageText(wording ? kernel_snapshot().holding.words : "");
    if (wording) useDockStore.getState().setDockMode(DockMode.Message);
  }, [wording]);

  function changeText(text: string) {
    setMessageText(text);
    if (wording && markerTextFits(text.trim())) draftMarkerWords(text.trim());
  }

  function sendMessage() {
    const text = messageText.trim();
    if (!canSend || !fits || isSending) return;
    setIsSending(true);
    setTimeout(() => {
      setMessageText("");
      setIsSending(false);
      if (!wording) {
        appendChatLine(text);
        inputRef.current?.focus();
        return;
      }
      putMarker();
      // Put up, the palette comes back for the next.
      useDockStore.getState().setDockMode(DockMode.Palette);
    }, FADE.durationMs);
  }

  const canSend = messageText.trim() !== "";
  // A line too long for the kernel is refused whole, never cut.
  const fits = wording ? markerTextFits(messageText.trim()) : chatFits(messageText.trim());

  return { wording, messageText, isSending, inputRef, changeText, sendMessage, canSend, fits };
}

export function Message({ message, width }: { message: ReturnType<typeof useMessage>; width: f64 }) {
  const { t } = useTranslation();
  const { wording, messageText, isSending, inputRef, changeText, sendMessage, canSend, fits } = message;

  return (
    <View style={[styles.message, { width }]}>
      <View style={styles.messageIcon}>
        <Glyph glyph="bubble.left" size={chrome.glyph} color={LABEL.primary} />
      </View>
      <Motion to={{ opacity: isSending ? 0 : 1 }} transition={FADE} style={styles.fieldFill}>
        <TextField
          ref={inputRef}
          value={messageText}
          onChangeText={changeText}
          onSubmit={sendMessage}
          placeholder={wording ? t("world.dock.pin_placeholder") : t("world.dock.message_placeholder")}
          placeholderTextColor={LABEL.placeholder}
          style={styles.messageInput}
        />
      </Motion>
      <Presence show={canSend} from={{ opacity: 0, scale: 0.5 }} exit={{ opacity: 0 }} transition={SNAP} style={styles.slot}>
        <Pressable
          onPress={sendMessage}
          disabled={isSending || !fits}
          accessibilityRole="button"
          accessibilityLabel={wording ? t("world.dock.pin_done") : t("world.dock.send")}
          style={({ pressed }) => [styles.send, wording && styles.confirm, pressed && styles.sendPressed, !fits && styles.sendRefused]}
        >
          <Glyph glyph={wording ? "checkmark" : "arrow.up"} size={chrome.glyph} weight="bold" color={color.white} />
        </Pressable>
      </Presence>
    </View>
  );
}

const styles = StyleSheet.create({
  message: { flexDirection: "row", alignItems: "center", height: chrome.control, paddingRight: (chrome.control - chrome.target) / 2 },
  messageIcon: { width: chrome.control, height: chrome.control, alignItems: "center", justifyContent: "center" },
  fieldFill: { flex: 1, minWidth: 0 },
  messageInput: { width: "100%", color: LABEL.primary },
  send: { width: chrome.selection, height: chrome.selection, borderRadius: chrome.selection / 2, alignItems: "center", justifyContent: "center", backgroundColor: color.send },
  confirm: { backgroundColor: color.confirm },
  sendPressed: { opacity: 0.7 },
  sendRefused: { opacity: disabledContent },
  slot: SLOT,
});
