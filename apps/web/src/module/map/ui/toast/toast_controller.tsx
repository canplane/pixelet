// @pxl/web/module/map/ui/toast/toast_controller.tsx

import { useEffect, useRef } from "react";
import { message_key } from "@/module/map/ui/toast/message_key.ts";
import { AnimatePresence, motion } from "framer-motion";
import { CheckCircle2, Loader2, MapPinOff } from "lucide-react";
import { useTranslation } from "react-i18next";

import { on_toast } from "@/wasm/kernel.ts";

import { Toast } from "@/shared/ui/toast/toast.tsx";
import { toastCustom, toastDismiss } from "@/shared/ui/toast/store.ts";

const LoadingIcon = (
  <motion.div
    key="loading-icon"
    initial={{ opacity: 0, scale: 0.8 }}
    animate={{ opacity: 1, scale: 1 }}
    exit={{ opacity: 0, scale: 0.5 }}
    transition={{ duration: 0.2 }}
  >
    <Loader2 className="w-4 h-4 text-[#5B4636] animate-spin-slow stroke-[2.5px]" />
  </motion.div>
);

const SuccessIcon = (
  <motion.div
    key="success-icon"
    initial={{ opacity: 0, scale: 0.3, rotate: -20 }}
    animate={{ opacity: 1, scale: 1, rotate: 0 }}
    transition={{ type: "spring", damping: 12, stiffness: 260, mass: 0.8 }}
  >
    <CheckCircle2 className="w-4 h-4 text-[#34C759] stroke-[2.5px]" />
  </motion.div>
);

function StatusIcon({ children }: { children: React.ReactNode }) {
  return (
    <div className="relative flex items-center justify-center w-4 h-4">
      <AnimatePresence mode="wait" initial={false}>
        {children}
      </AnimatePresence>
    </div>
  );
}

/** The kernel's status toasts (crates/pxl/core/src/module/map/ui/toast/toast_controller.rs):
 * which show and what they say is the kernel's; this renders them. The message
 * is a key rendered through t() here, since i18next loads its catalogue over the
 * network and a resolved string would freeze before a later language change. */
export function ToastController() {
  const { t } = useTranslation();
  // t via a ref, not a dependency — react-i18next hands back a new function
  // identity on every language change, which would re-subscribe for nothing.
  const t_ref = useRef(t);
  t_ref.current = t;

  useEffect(() =>
    on_toast((e) => {
      if (!e.show) {
        toastDismiss(e.id);
        return;
      }
      const is_error = e.tone === 1;
      const is_success = e.tone === 2;
      toastCustom(e.id, ({ index, stackSize, isRemoving }) => (
        <Toast
          id={e.id}
          index={index}
          stackSize={stackSize}
          isRemoving={isRemoving}
          message={t_ref.current(message_key(e.message) ?? "")}
          statusColor={is_error ? "bg-red-500" : is_success ? "bg-[#34C759]" : "bg-[#34C759]/30"}
          icon={<StatusIcon>{is_error ? <MapPinOff key={`${e.id}-error-icon`} className="w-4 h-4 text-red-500" /> : is_success ? SuccessIcon : LoadingIcon}</StatusIcon>}
          backgroundColor={is_error ? "bg-red-50" : undefined}
          className={is_error ? "border-red-200 shadow-[0_6px_20px_rgba(220,38,38,0.15)]" : undefined}
        />
      ));
    }), []);

  return null;
}
