// @pxl/client/ui/shared/modal.ts

import { create } from "zustand";

export type Modal = "settings" | "drawer" | "search";

const store = create<{ asked: Modal | null }>()(() => ({ asked: null }));

export function openModal(modal: Modal): void {
  store.setState({ asked: modal });
}

export function closeModal(): void {
  store.setState({ asked: null });
}

export function modalOpen(): boolean {
  return store.getState().asked !== null;
}

export function useAskedModal(): Modal | null {
  return store((s) => s.asked);
}
