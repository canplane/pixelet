// @pxl/web/app/providers/modal_manager.tsx

import { useEffect, useState } from "react";

import type { ModalController, UIController } from "../types/ui.types.ts";

import { EditFooModal } from "@/features/manage_foo/ui/edit_foo_modal.tsx";
import type { EditFooProps } from "@/features/manage_foo/ui/edit_foo_modal.tsx";
import { SettingsModal, type SettingsModalProps } from "@/app/settings/settings_modal.tsx";

/**
 * Represents the currently active modal state.
 * `null` means no modal is visible.
 */
export type ModalState = EditFooProps | SettingsModalProps | null;

/** ModalManager props */
type ModalManagerProps = {
  merge_ui_controller: (o: Partial<UIController>) => void; // Merges modal-related UI actions into the global UI controller
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
  const { merge_ui_controller } = props;

  const [modal_state, set_modal_state] = useState<ModalState>(null);

  /**
   * Modal-specific UI controller
   *
   * This controller exposes imperative actions
   * (e.g. opening a modal from anywhere in the app)
   */
  const [modals_controller] = useState<ModalController>({
    open_update_foo_modal: (id) => set_modal_state({ type: "foo:update", params: { id } }),
    open_settings_modal: () => set_modal_state({ type: "settings" }),
  });

  /**
   * Register modal actions to the global UI controller
   */
  useEffect(() => {
    merge_ui_controller(modals_controller);
  }, [modals_controller, merge_ui_controller]);

  // no active modal
  if (!modal_state) {
    return null;
  }
  const { type } = modal_state;

  return (
    <>
      {type === "foo:update" && (
        <EditFooModal
          open={!!modal_state}
          on_open_change={(open: boolean) => !open && set_modal_state(null)}
          {...modal_state}
        />
      )}
      {type === "settings" && (
        <SettingsModal
          open={!!modal_state}
          on_open_change={(open: boolean) => !open && set_modal_state(null)}
          {...modal_state}
        />
      )}
    </>
  );
}
