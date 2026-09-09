// @pxl/web/app/providers/modal-manager.tsx

import { useEffect, useState } from "react";

import type { ModalController, UiController } from "../types/ui.types.ts";

import { EditFooModal } from "@/features/manage_foo/ui/edit_foo_modal.tsx";
import type { EditFooProps } from "@/features/manage_foo/ui/edit_foo_modal.tsx";
import { SettingsModal, type SettingsModalProps } from "@/app/settings/settings_modal.tsx";
import { DrawerModal, type DrawerModalProps } from "@/modules/map/ui/overlay/drawer_modal.tsx";
import { SearchModal, type SearchModalProps } from "@/modules/map/ui/overlay/search_modal.tsx";

/**
 * Represents the currently active modal state.
 * `null` means no modal is visible.
 */
export type ModalState = EditFooProps | SettingsModalProps | DrawerModalProps | SearchModalProps | null;

/** ModalManager props */
type ModalManagerProps = {
  mergeUiController: (o: Partial<UiController>) => void; // Merges modal-related UI actions into the global UI controller
};

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

  // no active modal
  if (!modalState) {
    return null;
  }
  const { type } = modalState;

  return (
    <>
      {type === "foo:update" && (
        <EditFooModal
          open={!!modalState}
          onOpenChange={(open: boolean) => !open && setModalState(null)}
          {...modalState}
        />
      )}
      {type === "settings" && (
        <SettingsModal
          open={!!modalState}
          onOpenChange={(open: boolean) => !open && setModalState(null)}
          {...modalState}
        />
      )}
      {type === "drawer" && (
        <DrawerModal
          open={!!modalState}
          onOpenChange={(open: boolean) => !open && setModalState(null)}
          {...modalState}
        />
      )}
      {type === "search" && (
        <SearchModal
          open={!!modalState}
          onOpenChange={(open: boolean) => !open && setModalState(null)}
          {...modalState}
        />
      )}
    </>
  );
}
