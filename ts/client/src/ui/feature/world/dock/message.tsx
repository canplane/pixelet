// @pxl/client/ui/feature/world/dock/message.tsx

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Pressable,
  StyleSheet,
  type TextInputInstance,
  View,
} from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Glyph } from "@/ui/shared/component/glyph.tsx";
import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import { Presence } from "@/ui/shared/component/motion/presence.tsx";
import { FADE, SNAP } from "@/ui/shared/component/motion/transition.ts";
import { putKeyboardAway } from "@/ui/shared/component/keyboard.tsx";
import { LABEL } from "@/ui/shared/component/palette.ts";
import { TextField } from "@/ui/shared/component/text-field.tsx";
import {
  CHROME,
  COLOR,
  DISABLED_CONTENT,
} from "@/ui/shared/component/theme.ts";
import { useCore } from "@/ui/shared/watch.ts";
import {
  CHAT_TEXT_CHARS,
  MARKER_TEXT_CHARS,
} from "@/ui/shared/asset/marker.spec.ts";
import { appendChatLine, DockMode, textFits, setDockMode } from "./store.ts";
import { SLOT } from "./strip.tsx";
import * as edit from "@/service/core/edit.ts";
import * as overlay from "@/service/core/overlay.ts";

/** The dock holds this state, so that it is there whether or not the cards are
 * drawn yet. */
export function useMessage(mode: DockMode) {
  const drafting = useCore((s) => s.drafting);
  const [messageText, setMessageText] = useState("");
  const [isSending, setIsSending] = useState(false);
  const inputRef = useRef<TextInputInstance | null>(null);

  // The keyboard is up exactly while the dock is in message mode. A marker's
  // draft exists only in message mode, so leaving the mode cancels it.
  useEffect(() => {
    if (mode === DockMode.MESSAGE) inputRef.current?.focus();
    else {
      putKeyboardAway();
      if (drafting) edit.markerCancel();
    }
  }, [mode]);

  // When a draft opens, the dock turns to message mode and the field takes the
  // draft's text. From then on the field owns the text and sends the core each
  // change. When the draft closes, the field is emptied for the chat.
  useEffect(() => {
    setMessageText(drafting ? overlay.currentDraft() : "");
    if (drafting) setDockMode(DockMode.MESSAGE);
  }, [drafting]);

  function changeText(text: string) {
    setMessageText(text);
    if (drafting) edit.markerText(text.trim());
  }

  function sendMessage() {
    const text = messageText.trim();
    if (!canSend || !fits || isSending) return;
    setIsSending(true);
    setTimeout(() => {
      if (!drafting) {
        // The line stays in the field where the core refuses it.
        void appendChatLine(text).then((taken) => {
          setIsSending(false);
          if (!taken) return;
          setMessageText("");
          inputRef.current?.focus();
        });
        return;
      }
      setMessageText("");
      setIsSending(false);
      edit.markerCommit();
      setDockMode(DockMode.PALETTE);
    }, FADE.durationMs);
  }

  const canSend = messageText.trim() !== "";
  // A text past its kind's cap can be neither put up nor said.
  const fits = textFits(
    messageText,
    drafting ? MARKER_TEXT_CHARS : CHAT_TEXT_CHARS,
  );

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
    <View style={[STYLE.message, { width }]}>
      <View style={STYLE.messageIcon}>
        <Glyph glyph="bubble.left" size={CHROME.glyph} color={LABEL.primary} />
      </View>
      <Motion
        to={{ opacity: isSending ? 0 : 1 }}
        transition={FADE}
        style={STYLE.fieldFill}
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
          style={STYLE.messageInput}
        />
      </Motion>
      <Presence
        show={canSend}
        from={{ opacity: 0, scale: 0.5 }}
        exit={{ opacity: 0 }}
        transition={SNAP}
        style={STYLE.slot}
      >
        <Pressable
          onPress={sendMessage}
          disabled={isSending || !fits}
          accessibilityRole="button"
          accessibilityLabel={
            drafting ? t("world.dock.put_pin") : t("world.dock.send")
          }
          style={({ pressed }) => [
            STYLE.send,
            drafting && STYLE.confirm,
            pressed && STYLE.sendPressed,
            !fits && STYLE.sendRefused,
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
  messageIcon: {
    width: CHROME.control,
    height: CHROME.control,
    alignItems: "center",
    justifyContent: "center",
  },
  fieldFill: { flex: 1, minWidth: 0 },
  messageInput: { width: "100%", color: LABEL.primary },
  send: {
    width: CHROME.selection,
    height: CHROME.selection,
    borderRadius: CHROME.selection / 2,
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: COLOR.send,
  },
  confirm: { backgroundColor: COLOR.confirm },
  sendPressed: { opacity: 0.7 },
  sendRefused: { opacity: DISABLED_CONTENT },
  slot: SLOT,
});
