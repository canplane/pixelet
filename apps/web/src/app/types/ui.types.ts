// @pxl/web/app/types/ui.types.ts

/** Base props shared by modal components */
export type BaseModalProps = {
  open: boolean; // Whether the modal is open
  on_open_change: (open: boolean) => void; // Callback invoked when the open state changes
};

/**
 * UI controller interfaces
 *
 * Controllers define imperative UI actions
 * that are triggered outside of the component tree
 * (e.g. opening modals, panels, or overlays).
 */

/** Modal-related UI actions */
export type ModalController = {
  open_update_foo_modal: (id: number) => void; // DUMMY: Open the "Update Foo" modal for a specific entity
};

/**
 * Root UI controller interface
 *
 * This type aggregates all UI-related controllers
 * to provide a single access point for imperative UI actions.
 */
export type UIController = {} & ModalController;
