// @pxl/web/app/type/ui.type.ts

export interface BaseModalProps {
  onClose: () => void;
}

export interface ModalController {
  openSettingsModal: () => void;
  openDrawerModal: () => void;
  openSearchModal: () => void;
}

export type UiController = ModalController;
