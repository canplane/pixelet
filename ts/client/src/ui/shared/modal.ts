// @pxl/client/ui/shared/modal.ts

import { create } from "zustand";

export type Modal = "settings" | "drawer" | "search";

const store = create<{ asked: Modal | null }>()(() => ({ asked: null }));

export function open_modal(modal: Modal): void {
  store.setState({ asked: modal });
}

export function close_modal(): void {
  store.setState({ asked: null });
}

export function modal_open(): boolean {
  return store.getState().asked !== null;
}

export function useAskedModal(): Modal | null {
  return store((s) => s.asked);
}
