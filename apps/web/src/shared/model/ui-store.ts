import { create } from 'zustand';

interface UiState {}

export const useUiStore = create<UiState>((set) => ({}));
