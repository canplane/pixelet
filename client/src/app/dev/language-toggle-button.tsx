// @pxl/web/app/dev/language-toggle-button.tsx

import { useTranslation } from "react-i18next";

/** Button to toggle language */
export function LanguageToggleButton() {
  const { i18n } = useTranslation();
  const changeLanguage = (lng: string) => i18n.changeLanguage(lng);

  return (
    <div className="bg-white rounded flex items-center">
      <button
        type="button"
        className="w-8 h-8"
        onClick={(e) => changeLanguage("ko")}
      >
        {"한"}
      </button>
      <button
        type="button"
        className="w-8 h-8"
        onClick={(e) => changeLanguage("en")}
      >
        {"A"}
      </button>
    </div>
  );
}
