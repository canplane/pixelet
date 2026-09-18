// @pxl/web/app/type/ui.type.ts

/** Base props shared by modal components */
export type BaseModalProps = {
  open: boolean; // Whether the modal is open
  onOpenChange: (open: boolean) => void; // Callback invoked when the open state changes
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
  openUpdateFooModal: (id: number) => void; // DUMMY: Open the "Update Foo" modal for a specific entity
  openSettingsModal: () => void; // Opens the settings modal (rendering effects / debug / license)
  openDrawerModal: () => void; // Opens the full cube/decor/character catalog drawer (mode_bar.tsx's own hotbar stays separate)
  openSearchModal: () => void; // Opens forward search (right-side sheet, triggered from CurrentAddress)
};

/**
 * Root UI controller interface
 *
 * This type aggregates all UI-related controllers
 * to provide a single access point for imperative UI actions.
 */
export type UiController = ModalController;
