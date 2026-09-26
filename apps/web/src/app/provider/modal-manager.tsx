// @pxl/web/app/provider/modal-manager.tsx

import { useEffect, useState } from "react";

import type { ModalController, UiController } from "../type/ui.type.ts";

import { PanelPresence } from "@/shared/ui/panel.tsx";

import { EditFooModal } from "@/feature/placeholder/ui/edit-foo-modal.tsx";
import type { EditFooProps } from "@/feature/placeholder/ui/edit-foo-modal.tsx";
import { SettingsModal, type SettingsModalProps } from "@/feature/setting/modal.tsx";
import { DrawerModal, type DrawerModalProps } from "@/feature/map/ui/overlay/drawer-modal.tsx";
import { SearchModal, type SearchModalProps } from "@/feature/search/modal.tsx";

/**
 * Represents the currently active modal state.
 * `null` means no modal is visible.
 */
export type ModalState = EditFooProps | SettingsModalProps | DrawerModalProps | SearchModalProps | null;

/** ModalManager props */
interface ModalManagerProps {
  mergeUiController: (o: Partial<UiController>) => void; // Merges modal-related UI actions into the global UI controller
}

/**
 * Global modal host
 *
 * This component is responsible for:
 * - Managing modal visibility and state
 * - Registering imperative modal actions to the UI controller
 * - Rendering active modals at the application root level
 */
export function ModalManager(props: ModalManagerProps) {
  const { mergeUiController } = props;

  const [modalState, setModalState] = useState<ModalState>(null);

  /**
   * Modal-specific UI controller
   *
   * This controller exposes imperative actions
   * (e.g. opening a modal from anywhere in the app)
   */
  const [modalsController] = useState<ModalController>({
    openUpdateFooModal: (id) => setModalState({ type: "foo:update", params: { id } }),
    openSettingsModal: () => setModalState({ type: "settings" }),
    openDrawerModal: () => setModalState({ type: "drawer" }),
    openSearchModal: () => setModalState({ type: "search" }),
  });

  /**
   * Register modal actions to the global UI controller
   */
  useEffect(() => {
    mergeUiController(modalsController);
  }, [modalsController, mergeUiController]);

  // What is drawn: the modal asked for, or the last one while it slides
  // away. A modal asked for over one still leaving replaces it at once.
  const [shown, setShown] = useState<ModalState>(null);
  if (modalState !== null && modalState !== shown) setShown(modalState);
  const open = modalState !== null;

  if (!shown) {
    return null;
  }
  const { type } = shown;
  const onOpenChange = (next: boolean) => !next && setModalState(null);

  return (
    <PanelPresence.Provider value={{ open, gone: () => setShown(null) }}>
      {type === "foo:update" && (
        <EditFooModal
          open={open}
          onOpenChange={onOpenChange}
          {...shown}
        />
      )}
      {type === "settings" && (
        <SettingsModal
          open={open}
          onOpenChange={onOpenChange}
          {...shown}
        />
      )}
      {type === "drawer" && (
        <DrawerModal
          open={open}
          onOpenChange={onOpenChange}
          {...shown}
        />
      )}
      {type === "search" && (
        <SearchModal
          open={open}
          onOpenChange={onOpenChange}
          {...shown}
        />
      )}
    </PanelPresence.Provider>
  );
}
