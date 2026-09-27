// @pxl/web/app/type/ui.type.ts

/** What every modal is handed: the way to close it. Whether it is open is
 * its panel's to hear (shared/ui/panel.tsx's PanelPresence). */
export interface BaseModalProps {
  onClose: () => void;
}

/**
 * UI controller interfaces
 *
 * Controllers define imperative UI actions
 * that are triggered outside of the component tree
 * (e.g. opening modals, panels, or overlays).
 */

/** Modal-related UI actions */
export interface ModalController {
  openSettingsModal: () => void; // Opens the settings modal (rendering effects / debug / license)
  openDrawerModal: () => void; // Opens the full cube/decor/character catalog drawer (mode-bar.tsx's own hotbar stays separate)
  openSearchModal: () => void; // Opens forward search (a sheet, triggered from CurrentAddress)
}

/**
 * Root UI controller interface
 *
 * This type aggregates all UI-related controllers
 * to provide a single access point for imperative UI actions.
 */
export type UiController = ModalController;
