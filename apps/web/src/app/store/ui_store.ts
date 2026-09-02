// @pxl/web/app/store/ui_store.ts

import { create } from "zustand";

import type { f64 } from "@xpute/core/kit/word.ts";
import type { ObjectColorId } from "@pxl/shared/spec/cell/objects.ts";
import { DEFAULT_OBJECT_SIZE } from "@/modules/world/objects/size.ts";

export type ActiveTool = "pointer" | "object" | "eraser";

// ts (Date.now() at append) lets ChatBubble derive its own remaining show
// time from wall-clock, not from when its own component happened to mount —
// without it, a character swap (Walker remounts under a new variant.key,
// see companion.tsx) resets ChatBubble's local state and it has no way to
// tell an old, already-dismissed line apart from a brand new one.
export type ChatLine = { id: number; text: string; ts: number };

const MAX_CHAT_LINES = 10;
let nextChatLineID = 0;

type UiState = {
  activeTool: ActiveTool;
  set_activeTool: (activeTool: ActiveTool) => void;

  object_color_id: ObjectColorId;
  set_object_color_id: (object_color_id: ObjectColorId) => void;

  object_size: f64;
  set_object_size: (object_size: f64) => void;

  chatLines: ChatLine[];
  append_chat_line: (text: string) => void;
};

export const useUiStore = create<UiState>((set) => ({
  activeTool: "pointer",
  set_activeTool: (activeTool) => set({ activeTool }),

  object_color_id: "red",
  set_object_color_id: (object_color_id) => set({ object_color_id }),

  object_size: DEFAULT_OBJECT_SIZE,
  set_object_size: (object_size) => set({ object_size }),

  chatLines: [],
  append_chat_line: (text) =>
    set((state) => ({
      chatLines: [...state.chatLines, { id: nextChatLineID++, text, ts: Date.now() }].slice(-MAX_CHAT_LINES),
    })),
}));
