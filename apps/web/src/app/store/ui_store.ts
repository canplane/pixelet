// @pxl/web/app/store/ui_store.ts

import { create } from "zustand";

import type { f64 } from "@xpute/core/kit/word.ts";
import type { DecorId, PlaceableCubeId } from "@pxl/shared/spec/cell/mod.ts";
import { DEFAULT_OBJECT_SIZE } from "@/modules/world/objects/size.ts";

export type ActiveTool = "pointer" | "object" | "decor" | "eraser";

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

  // Widened to the full placeable catalog (2026-09-06, palette_modal.tsx's
  // own terrain-styled entries alongside mode_bar.tsx's original 15 flat
  // colors) — the hotbar only ever sets this to the narrower CubeColorId
  // subset, but the field itself has to hold whatever the modal picks too.
  object_color_id: PlaceableCubeId;
  set_object_color_id: (object_color_id: PlaceableCubeId) => void;

  object_size: f64;
  set_object_size: (object_size: f64) => void;

  decor_id: DecorId;
  set_decor_id: (decor_id: DecorId) => void;

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

  decor_id: "tree",
  set_decor_id: (decor_id) => set({ decor_id }),

  chatLines: [],
  append_chat_line: (text) =>
    set((state) => ({
      chatLines: [...state.chatLines, { id: nextChatLineID++, text, ts: Date.now() }].slice(-MAX_CHAT_LINES),
    })),
}));
